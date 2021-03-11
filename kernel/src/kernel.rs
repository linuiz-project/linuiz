#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt, once_cell)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod drivers;
mod logging;
mod pic8259;
mod timer;

use core::ffi::c_void;
use libkernel::{
    structures::{acpi::RDSPDescriptor2, SystemConfigTableEntry},
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

static mut SERIAL_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);
static KERNEL_ALLOCATOR: libkernel::memory::BlockAllocator =
    libkernel::memory::BlockAllocator::new();

#[no_mangle]
#[export_name = "_start"]
extern "efiapi" fn kernel_main(
    boot_info: BootInfo<libkernel::memory::UEFIMemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    unsafe {
        SERIAL_OUT.init(drivers::io::SerialSpeed::S115200);
        drivers::io::set_stdout(&mut SERIAL_OUT);
    }

    match crate::logging::init_logger(crate::logging::LoggingModes::STDOUT, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
            debug!("Minimum logging level configured as: {:?}", get_log_level());
        }
        Err(error) => panic!("{}", error),
    }

    unsafe {
        let config_table = boot_info.config_table();
        info!("Initializing system configuration table.");
        libkernel::structures::init_system_config_table(config_table.as_ptr(), config_table.len());
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
    init_memory(boot_info);
    init_apic();

    let rdsp: &RDSPDescriptor2 = unsafe {
        libkernel::structures::system_config_table()
            .iter()
            .find(|entry| entry.guid() == libkernel::structures::acpi::ACPI2_GUID)
            .unwrap()
            .as_ref()
    };

    let xsdt = rdsp.xsdt();

    for entry in xsdt.iter() {
        use libkernel::structures::acpi::XSDTEntry;

        if let XSDTEntry::MCFG(mcfg) = entry {
            info!("MCFG FOUND");

            for mcfg_entry in mcfg.iter() {
                info!("{:?}", mcfg_entry);

                for pci_bus in mcfg_entry.iter() {}
            }
        } else if let XSDTEntry::APIC(madt) = entry {
            info!("MADT FOUND");

            for madt_entry in madt.iter() {
                info!("{:?}", madt_entry);
            }
        }
    }

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_memory(
    boot_info: BootInfo<libkernel::memory::UEFIMemoryDescriptor, SystemConfigTableEntry>,
) {
    use libkernel::memory::{global_memory, FrameState};

    info!("Initializing global memory.");
    unsafe { libkernel::memory::init_global_memory(boot_info.memory_map()) };

    debug!("Reserving frames from relevant UEFI memory descriptors.");

    let mut stack_frames = core::lazy::OnceCell::new();
    let mut last_frame_end = 0;
    for descriptor in boot_info.memory_map() {
        let cur_frame_start = descriptor.phys_start.as_usize() / 0x1000;
        let new_frame_end = cur_frame_start + (descriptor.page_count as usize);

        // Checks for 'holes' in system memory which we shouldn't try to allocate to.
        if last_frame_end < cur_frame_start {
            unsafe {
                global_memory()
                    .acquire_frames(last_frame_end..cur_frame_start, FrameState::NonUsable)
                    .unwrap()
            };
        }

        // Reserve descriptor properly, and acquire stack frames if applicable.
        if descriptor.should_reserve() {
            let frame_range = cur_frame_start..new_frame_end;

            if descriptor.is_stack_descriptor() {
                debug!("Identified stack frames: {:?}", frame_range);
                let descriptor_stack_frames = unsafe {
                    global_memory()
                        .acquire_frames(frame_range, FrameState::Reserved)
                        .unwrap()
                };

                stack_frames
                    .set(descriptor_stack_frames)
                    .expect("multiple stack descriptors found");
            } else {
                unsafe {
                    global_memory()
                        .acquire_frames(frame_range, FrameState::Reserved)
                        .unwrap()
                };
            }
        }

        last_frame_end = new_frame_end;
    }

    info!("Initializing kernel default allocator.");
    unsafe {
        KERNEL_ALLOCATOR.init(stack_frames.get_mut().unwrap());
        libkernel::memory::malloc::set(&KERNEL_ALLOCATOR);
    }

    info!("Global memory & the kernel global allocator have been initialized.");
}

fn init_apic() {
    use libkernel::structures::{
        apic::{APICRegister, APICTimerDivisor, APICTimerMode},
        idt,
    };

    const APIC_TIMER_IVT: u8 = 48;

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
    apic.timer().set_vector(APIC_TIMER_IVT);
    apic.timer().set_mode(APICTimerMode::OneShot);
    apic.timer().set_masked(false);

    debug!("Determining APIC timer frequency using PIT windowing.");
    apic[APICRegister::TimerDivisor] = APICTimerDivisor::Div1 as u32;
    apic[APICRegister::TimerInitialCount] = u32::MAX;

    timer.wait();

    apic.timer().set_masked(true);
    apic[APICRegister::TimerInitialCount] = u32::MAX - apic[APICRegister::TimeCurrentCount];
    apic[APICRegister::TimerDivisor] = APICTimerDivisor::Div1 as u32;

    debug!("Disabling 8259 emulated PIC.");
    libkernel::instructions::interrupts::without_interrupts(|| unsafe {
        crate::pic8259::disable()
    });
    debug!("Updating IDT timer interrupt entry to local APIC-enabled function.");
    idt::set_interrupt_handler(APIC_TIMER_IVT, timer::apic_timer_handler);
    debug!("Unmasking local APIC timer interrupt (it will fire now!).");
    apic.timer().set_mode(APICTimerMode::Periodic);
    apic.timer().set_masked(false);

    info!(
        "Core-local APIC configured and enabled (freq {}MHz).",
        apic[APICRegister::TimerInitialCount] / 1000
    );
}

pub unsafe fn init_global_memory(memory_map: &[crate::memory::UEFIMemoryDescriptor]) {
    assert!(
        !GLOBAL_MEMORY.has_allocator(),
        "global memory has already been initialized"
    );

    // calculates total system memory
    let total_memory = memory_map
        .iter()
        .max_by_key(|descriptor| descriptor.phys_start)
        .map(|descriptor| {
            (descriptor.phys_start + ((descriptor.page_count as usize) * 0x1000)).as_usize()
        })
        .expect("no descriptor with max value");

    info!(
        "Global memory will represent {} MB ({} bytes) of system memory.",
        crate::memory::to_mibibytes(total_memory),
        total_memory
    );

    let frame_alloc_frame_count = FrameAllocator::frame_count_hint(total_memory);
    let frame_alloc_ptr = memory_map
        .iter()
        .filter(|descriptor| descriptor.ty == crate::memory::UEFIMemoryType::CONVENTIONAL)
        .find(|descriptor| descriptor.page_count >= (frame_alloc_frame_count as u64))
        .map(|descriptor| descriptor.phys_start.as_usize() as *mut _)
        .expect("failed to find viable memory descriptor for memory map.");

    debug!("Configuring and assigning global memory instance.");
    libkernel::memory::falloc::set(FrameAllocator::from_ptr(frame_alloc_ptr, total_memory));
}
