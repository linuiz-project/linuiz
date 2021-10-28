#![no_std]
#![no_main]
#![feature(
    asm,
    abi_efiapi,
    abi_x86_interrupt,
    once_cell,
    const_mut_refs,
    raw_ref_op
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod block_malloc;
mod drivers;
mod logging;
mod pic8259;
mod timer;

use core::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use libkernel::{
    acpi::{rdsp::xsdt::madt::MADT, SystemConfigTableEntry},
    addr_ty::Physical,
    io::pci::PCIeHostBridge,
    memory::{falloc, UEFIMemoryDescriptor},
    Address, BootInfo, LinkerSymbol,
};
use x86_64::instructions::segmentation::Segment;

extern "C" {
    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;

    static __rodata_start: LinkerSymbol;
    static __rodata_end: LinkerSymbol;

    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;

    static __bss_start: LinkerSymbol;
    static __bss_end: LinkerSymbol;

    static __ap_trampoline_start: LinkerSymbol;
    static __ap_trampoline_end: LinkerSymbol;

    static __kernel_pml4: LinkerSymbol;
    static __cpu_count: LinkerSymbol;
}

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Trace
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

static mut CON_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);
// static mut CON_OUT: drivers::io::QEMUE9 = drivers::io::QEMUE9::new();
static KERNEL_MALLOC: block_malloc::BlockAllocator = block_malloc::BlockAllocator::new();
static TRACE_ENABLED_PATHS: [&str; 2] = ["kernel::drivers::ahci", "libkernel::memory::falloc"];
static CPU_INIT: AtomicBool = AtomicBool::new(true);

#[no_mangle]
#[export_name = "_entry"]
extern "efiapi" fn kernel_stage_zero(
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

    // `boot_info` will not be usable after initalizing the global allocator,
    //   due to the stack being moved in virtual memory.
    unsafe {
        let memory_map = boot_info.memory_map();
        init_falloc(memory_map);
        init_system_config_table(boot_info.config_table());

        info!("Initializing kernel default allocator.");
        KERNEL_MALLOC.init(reserve_kernel_stack(memory_map));
        // Move the current PML4 into the global processor reference.
        // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
        __kernel_pml4
            .as_mut_ptr::<u32>()
            .write(libkernel::registers::CR3::read().0.as_usize() as u32);

        libkernel::memory::malloc::set(&KERNEL_MALLOC);

        debug!("Ensuring relevant kernel sections are read-only.");
        let malloc = libkernel::memory::malloc::get();
        for page in unsafe {
            (__text_start.as_page()..__text_end.as_page())
                .chain(__rodata_start.as_page()..__rodata_end.as_page())
        } {
            malloc.set_page_attributes(
                &page,
                libkernel::memory::paging::PageAttributes::WRITABLE,
                libkernel::memory::paging::AttributeModify::Remove,
            );
        }

        debug!("Kernel memory intiialization complete.");
    }

    // unsafe {
    //     debug!(
    //         "Configuring GDT:\tCode: 0x{:X}\tData: 0x{:X}\tTSS: 0x{:X}",
    //         gdt::code(),
    //         gdt::data(),
    //         gdt::tss()
    //     );

    //     use libkernel::structures::gdt;

    //     gdt::init();
    //     info!("Successfully loaded GDT & configured segment registers.");
    // }

    // libkernel::structures::idt::init();
    // libkernel::structures::idt::load();
    // info!("Successfully initialized IDT.");

    _startup()
}

fn kernel_stage_one() -> ! {
    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    let is_bsp = libkernel::cpu::LPU_COUNT.load(Ordering::Acquire) == 0;

    // Wait for CPU_INIT to be possible, and then allow BSP to continue.
    while unsafe { !CPU_INIT.load(core::sync::atomic::Ordering::Acquire) } {}
    libkernel::structures::gdt::init();
    libkernel::structures::idt::load();
    libkernel::cpu::auto_init_lpu();
    init_apic();
    CPU_INIT.store(false, Ordering::Release);

    // If this is the BSP, wake other cores.
    if is_bsp {
        use libkernel::{
            acpi::rdsp::xsdt::{madt::*, LAZY_XSDT},
            structures::apic,
        };

        // Initialize other CPUs
        info!("Searching for additional processor cores...");
        let lapic = libkernel::cpu::lpu().apic();
        let icr = lapic.interrupt_command_register();
        if let Ok(madt) = LAZY_XSDT.find_sub_table::<MADT>() {
            for interrupt_device in madt.iter() {
                if let InterruptDevice::LocalAPIC(lapic_other) = interrupt_device {
                    if lapic.id() != lapic_other.id() {
                        info!("Identified additional processor core: {}", lapic_other.id());

                        let ap_trampoline_page_index =
                            unsafe { __ap_trampoline_start.as_page().index() } as u8;

                        icr.send_init(lapic_other.id());
                        icr.wait_pending();

                        icr.send_sipi(ap_trampoline_page_index, lapic_other.id());
                        icr.wait_pending();
                        icr.send_sipi(ap_trampoline_page_index, lapic_other.id());
                        icr.wait_pending();

                        // Allow CPU to initialize, and then wait for its initialization to complete.
                        CPU_INIT.store(true, Ordering::Release);
                        while unsafe { CPU_INIT.load(core::sync::atomic::Ordering::Acquire) } {}
                    }
                }
            }
        }
    }

    info!(
        "CURRENT CPU COUNT: {}",
        libkernel::cpu::LPU_COUNT.load(Ordering::Acquire)
    );

    // TODO create function for automatigically creating a new stack

    libkernel::instructions::hlt_indefinite()
}

pub unsafe fn init_falloc(memory_map: &[UEFIMemoryDescriptor]) {
    info!("Initializing kernel frame allocator.");

    // calculates total system memory
    let total_falloc_memory = memory_map
        .iter()
        .filter(|descriptor| !descriptor.should_reserve())
        .max_by_key(|descriptor| descriptor.phys_start)
        .map(|descriptor| {
            (descriptor.phys_start + ((descriptor.page_count as usize) * 0x1000)).as_usize()
        })
        .expect("no descriptor with max value");

    let total_phys_memory = memory_map
        .iter()
        .filter(|descriptor| !descriptor.should_reserve())
        .map(|descriptor| (descriptor.page_count as usize) * 0x1000)
        .sum::<usize>();
    info!(
        "Kernel frame allocator will represent {} MB ({} bytes) of system memory.",
        libkernel::memory::to_mibibytes(total_phys_memory),
        total_phys_memory
    );

    let frame_alloc_frame_count =
        falloc::FrameAllocator::frame_count_hint(total_falloc_memory) as u64;
    let frame_alloc_ptr = memory_map
        .iter()
        .filter(|descriptor| descriptor.ty == libkernel::memory::UEFIMemoryType::CONVENTIONAL)
        .find(|descriptor| descriptor.page_count >= frame_alloc_frame_count)
        .map(|descriptor| descriptor.phys_start.as_usize() as *mut _)
        .expect("failed to find viable memory descriptor for memory map");

    falloc::load(frame_alloc_ptr, total_falloc_memory);
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
                        falloc::FrameState::Reserved,
                    )
                    .unwrap()
            };
        }

        // Reserve descriptor properly, and acquire stack frames if applicable.
        if descriptor.should_reserve() {
            let descriptor_frames = unsafe {
                falloc::get()
                    .acquire_frames(frame_start, frame_count, falloc::FrameState::Reserved)
                    .unwrap()
            };

            if descriptor.is_stack_descriptor() {
                debug!("Identified stack frames: {}:{}", frame_start, frame_count);

                stack_frames
                    .set(descriptor_frames)
                    .expect("multiple stack descriptors found");
            }
        }

        last_frame_end = frame_start + frame_count;
    }

    stack_frames.take().expect("no stack frames found")
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
                .acquire_frame(index, falloc::FrameState::Reserved)
                .unwrap();
        }
    }
}

fn init_apic() {
    use libkernel::structures::{apic, idt};

    let apic = &libkernel::cpu::lpu().apic();

    unsafe {
        debug!("Resetting and enabling local APIC (it may have already been enabled).");
        apic.reset();
        apic.sw_enable();
        apic.set_spurious_vector(u8::MAX);
    }

    let timer = timer::Timer::new(1);

    debug!("Configuring APIC timer state.");
    apic.write_register(
        apic::Register::TimerDivisor,
        apic::TimerDivisor::Div1 as u32,
    );
    apic.write_register(apic::Register::TimerInitialCount, u32::MAX);
    apic.timer().set_mode(apic::TimerMode::OneShot);

    crate::pic8259::enable();
    info!("Successfully initialized PIC.");
    info!("Configuring PIT frequency to 1000Hz.");
    crate::pic8259::pit::set_timer_freq(
        crate::timer::FREQUENCY as u32,
        crate::pic8259::pit::OperatingMode::RateGenerator,
    );
    debug!("Setting timer interrupt handler and enabling interrupts.");
    idt::set_interrupt_handler(32, crate::timer::tick_handler);
    libkernel::instructions::interrupts::enable();

    debug!("Determining APIC timer frequency using PIT windowing.");
    apic.timer().set_masked(false);

    timer.wait();

    apic.timer().set_masked(true);
    let timer_count = apic.read_register(apic::Register::TimerCurrentCount);
    info!(
        "Determined core crystal clock frequency: {}MHz",
        timer_count / 1000000
    );
    apic.write_register(apic::Register::TimerInitialCount, u32::MAX - timer_count);
    apic.write_register(
        apic::Register::TimerDivisor,
        apic::TimerDivisor::Div1 as u32,
    );

    debug!("Disabling 8259 emulated PIC.");
    libkernel::instructions::interrupts::without_interrupts(|| unsafe {
        crate::pic8259::disable()
    });

    debug!("Updating APIC register vectors and respective IDT entries.");
    apic.timer().set_vector(32);
    idt::set_interrupt_handler(32, timer::apic_tick_handler);
    apic.error().set_vector(58);
    idt::set_interrupt_handler(58, apic_error_handler);

    debug!("Unmasking APIC timer interrupt (it will fire now!).");
    apic.timer().set_mode(apic::TimerMode::Periodic);
    apic.timer().set_masked(false);

    info!("Core-local APIC configured and enabled.");
}

extern "x86-interrupt" fn apic_error_handler(_: libkernel::structures::idt::InterruptStackFrame) {
    let apic = &libkernel::cpu::lpu().apic();

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}
