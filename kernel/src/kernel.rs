#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt, once_cell, const_mut_refs)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod block_malloc;
mod drivers;
mod logging;
mod pic8259;
mod timer;

use alloc::vec::Vec;
use core::ffi::c_void;
use libkernel::{
    acpi::SystemConfigTableEntry,
    io::pci::PCIeHostBridge,
    memory::{falloc, UEFIMemoryDescriptor},
    BootInfo,
};

extern "C" {
    static _text_start: c_void;
    static _text_end: c_void;

    static _rodata_start: c_void;
    static _rodata_end: c_void;

    static _data_start: c_void;
    static _data_end: c_void;

    static _bss_start: c_void;
    static _bss_end: c_void;
}

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

static mut CON_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);
// static mut CON_OUT: drivers::io::QEMUE9 = drivers::io::QEMUE9::new();
static KERNEL_MALLOC: block_malloc::BlockAllocator = block_malloc::BlockAllocator::new();
static TRACE_ENABLED_PATHS: [&str; 2] = ["kernel::drivers::ahci", "libkernel::memory::falloc"];

#[no_mangle]
#[export_name = "_start"]
extern "efiapi" fn kernel_main(
    boot_info: BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    unsafe {
        CON_OUT.init(drivers::io::SerialSpeed::S115200);

        match drivers::io::set_stdout(&mut CON_OUT, get_log_level(), &TRACE_ENABLED_PATHS) {
            Ok(()) => {
                info!("Successfully loaded into kernel, with logging enabled.");
            }
            Err(_) => loop {},
        }
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();

    debug!(
        "Detected CPU features: {:?}",
        libkernel::instructions::cpu_features()
    );

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");

    // `boot_info` will not be usable after initalizing the global allocator,
    //   due to the stack being moved in virtual memory.
    unsafe {
        let memory_map = boot_info.memory_map();
        init_falloc(memory_map);
        init_system_config_table(boot_info.config_table());

        info!("Initializing kernel default allocator.");
        KERNEL_MALLOC.init(reserve_kernel_stack(memory_map));
        libkernel::memory::malloc::set(&KERNEL_MALLOC);
    }

    init_apic();

    let bridges: Vec<PCIeHostBridge> = libkernel::acpi::rdsp::xsdt::LAZY_XSDT
        .expect("xsdt does not exist")
        .find_sub_table::<libkernel::acpi::rdsp::xsdt::mcfg::MCFG>()
        .unwrap()
        .iter()
        .filter_map(|entry| {
            libkernel::io::pci::configure_host_bridge(entry).map_or(None, |bridge| Some(bridge))
        })
        .collect();

    debug!("Configuring PCIe devices.");
    for device_variant in bridges
        .iter()
        .flat_map(|host_bridge| host_bridge.iter())
        .filter(|bus| bus.has_devices())
        .flat_map(|bus| bus.iter())
    {
        use libkernel::io::pci::{PCIeDeviceClass, PCIeDeviceVariant};

        if let PCIeDeviceVariant::Standard(device) = device_variant {
            if device.class() == PCIeDeviceClass::MassStorageController {
                // if device.subclass() == 0x06 && device.program_interface() == 0x1 {
                //     debug!("Configuring AHCI driver.");

                //     let mut ahci = drivers::ahci::AHCI::from_pcie_device(&device);

                //     debug!("Configuring AHCI SATA ports.");
                //     for port in ahci.sata_ports() {
                //         port.configure();
                //         let buffer = port.read(0, 4);
                //         info!("{:?}", buffer);
                //     }
                // } else
                if device.subclass() == 0x08 {
                    unsafe {
                        // use crate::drivers::nvme::NVMECapabilities;
                        use libkernel::io::pci::standard::StandardRegister;

                        let reg_0_mmio = device[StandardRegister::Register0].as_ref().unwrap();
                        // let cap = reg_0_mmio.read::<NVMECapabilities>(0).unwrap().read();
                    }
                }

                use libkernel::io::pci::standard::PCICapablities;
                for capability in device.capabilities() {
                    if let PCICapablities::MSIX(msix) = capability {
                        info!("{:#?}", msix);
                        info!("{:#?}", msix.get_message_table(device))
                    }
                }
            }
        }
    }

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

pub unsafe fn init_falloc(memory_map: &[UEFIMemoryDescriptor]) {
    info!("Initializing kernel frame allocator.");

    // calculates total system memory
    let total_memory = memory_map
        .iter()
        .filter(|descriptor| !descriptor.should_reserve())
        .max_by_key(|descriptor| descriptor.phys_start)
        .map(|descriptor| {
            (descriptor.phys_start + ((descriptor.page_count as usize) * 0x1000)).as_usize()
        })
        .expect("no descriptor with max value");

    info!(
        "Kernel frame allocator will represent {} MB ({} bytes) of system memory.",
        libkernel::memory::to_mibibytes(total_memory),
        total_memory
    );

    let frame_alloc_frame_count = falloc::FrameAllocator::frame_count_hint(total_memory) as u64;
    let frame_alloc_ptr = memory_map
        .iter()
        .filter(|descriptor| descriptor.ty == libkernel::memory::UEFIMemoryType::CONVENTIONAL)
        .find(|descriptor| descriptor.page_count >= frame_alloc_frame_count)
        .map(|descriptor| descriptor.phys_start.as_usize() as *mut _)
        .expect("failed to find viable memory descriptor for memory map");

    falloc::load(frame_alloc_ptr, total_memory);
    debug!("Kernel frame allocator initialized.");
}

fn reserve_kernel_stack(memory_map: &[UEFIMemoryDescriptor]) -> libkernel::memory::FrameIterator {
    debug!("Allocating frames according to BIOS memory map.");

    let mut last_frame_end = 0;
    let mut stack_frames = core::lazy::OnceCell::<libkernel::memory::FrameIterator>::new();
    for descriptor in memory_map {
        let frame_start = descriptor.phys_start.frame_index();
        let frame_count = descriptor.page_count as usize;

        // Checks for 'holes' in system memory which we shouldn't try to allocate to.
        if last_frame_end < frame_start {
            unsafe {
                falloc::get()
                    .acquire_frames(
                        last_frame_end,
                        frame_start - last_frame_end,
                        falloc::FrameState::NonUsable,
                    )
                    .unwrap()
            };
        }

        // Reserve descriptor properly, and acquire stack frames if applicable.
        if descriptor.should_reserve() {
            let descriptor_stack_frames = unsafe {
                falloc::get()
                    .acquire_frames(frame_start, frame_count, falloc::FrameState::NonUsable)
                    .unwrap()
            };

            if descriptor.is_stack_descriptor() {
                debug!("Identified stack frames: {}:{}", frame_start, frame_count);

                stack_frames
                    .set(descriptor_stack_frames)
                    .expect("multiple stack descriptors found");
            }
        }

        last_frame_end = frame_start + frame_count;
    }

    stack_frames.take().unwrap()
}

fn init_system_config_table(config_table: &[SystemConfigTableEntry]) {
    info!("Initializing system configuration table.");
    let config_table_ptr = config_table.as_ptr();
    let config_table_entry_len = config_table.len();

    let frame_index = (config_table_ptr as usize) / 0x1000;
    let frame_count =
        (config_table_entry_len * core::mem::size_of::<SystemConfigTableEntry>()) / 0x1000;

    unsafe {
        // Assign system configuration table prior to reserving frames to ensure one doesn't already exist.
        libkernel::acpi::init_system_config_table(config_table_ptr, config_table_entry_len);

        let frame_range = frame_index..(frame_index + frame_count);
        debug!("System configuration table: {:?}", frame_range);
        let frame_allocator = falloc::get();
        for index in frame_range {
            frame_allocator
                .acquire_frame(index, falloc::FrameState::NonUsable)
                .unwrap();
        }
    }
}

fn init_apic() {
    use libkernel::structures::{
        apic::{APICRegister, APICTimerDivisor, APICTimerMode},
        idt,
    };

    crate::pic8259::enable();
    info!("Successfully initialized PIC.");
    info!("Configuring PIT frequency to 1000Hz.");
    crate::pic8259::set_timer_freq(crate::timer::TIMER_FREQUENCY as u32);
    debug!("Setting timer interrupt handler and enabling interrupts.");
    idt::set_interrupt_handler(32, crate::timer::tick_handler);
    libkernel::instructions::interrupts::enable();

    libkernel::structures::apic::load();
    let apic = libkernel::structures::apic::local_apic_mut().unwrap();

    unsafe {
        debug!("Resetting and enabling local APIC (it may have already been enabled).");
        apic.reset();
        apic.enable();
        apic.write_spurious(u8::MAX, true);
    }

    let timer = timer::Timer::new(crate::timer::TIMER_FREQUENCY / 1000);
    debug!("Configuring APIC timer state.");
    apic.timer().set_mode(APICTimerMode::OneShot);
    apic.timer().set_masked(false);

    debug!("Determining APIC timer frequency using PIT windowing.");
    apic.reg_mut(APICRegister::TimerDivisor)
        .write(APICTimerDivisor::Div1 as u32);
    apic.reg_mut(APICRegister::TimerInitialCount)
        .write(u32::MAX);

    timer.wait();

    apic.timer().set_masked(true);
    let timer_count = apic.reg(APICRegister::TimerCurrentCount).read();
    apic.reg_mut(APICRegister::TimerInitialCount)
        .write(u32::MAX - timer_count);
    apic.reg_mut(APICRegister::TimerDivisor)
        .write(APICTimerDivisor::Div1 as u32);

    debug!("Disabling 8259 emulated PIC.");
    libkernel::instructions::interrupts::without_interrupts(|| unsafe {
        crate::pic8259::disable()
    });

    debug!("Updating APIC register vectors and respective IDT entires.");
    apic.timer().set_vector(48);
    idt::set_interrupt_handler(48, timer::apic_tick_handler);
    apic.error().set_vector(58);
    idt::set_interrupt_handler(58, apic_error_handler);

    debug!("Unmasking APIC timer interrupt (it will fire now!).");
    apic.timer().set_mode(APICTimerMode::Periodic);
    apic.timer().set_masked(false);

    info!("Core-local APIC configured and enabled.");
}

extern "x86-interrupt" fn apic_error_handler(_: libkernel::structures::idt::InterruptStackFrame) {
    let apic = libkernel::structures::apic::local_apic_mut().unwrap();

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}
