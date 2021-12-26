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
extern crate libstd;

mod block_malloc;
mod drivers;
mod logging;
mod timer;

use libstd::{
    acpi::SystemConfigTableEntry,
    memory::{falloc, UEFIMemoryDescriptor},
    BootInfo, LinkerSymbol,
};

extern "C" {
    static __ap_trampoline_start: LinkerSymbol;
    static __ap_trampoline_end: LinkerSymbol;
    static __kernel_pml4: LinkerSymbol;

    static __ap_stack_bottom: LinkerSymbol;
    static __ap_stack_top: LinkerSymbol;

    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;

    static __rodata_start: LinkerSymbol;
    static __rodata_end: LinkerSymbol;

    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;

    static __bss_start: LinkerSymbol;
    static __bss_end: LinkerSymbol;
}

#[export_name = "__ap_stack_pointers"]
static mut AP_STACK_POINTERS: [usize; 256] = [0; 256];

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Trace
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

static mut CON_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);
static KERNEL_MALLOC: block_malloc::BlockAllocator = block_malloc::BlockAllocator::new();
static TRACE_ENABLED_PATHS: [&str; 1] = ["libstd::structures::apic::icr"];

#[no_mangle]
#[export_name = "_entry"]
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

    info!("Validating BootInfo struct.");
    boot_info.validate_magic();

    debug!(
        "Detected CPU features: {:?}",
        libstd::instructions::cpu_features()
    );

    // `boot_info` will not be usable after initalizing the global allocator,
    //   due to the stack being moved in virtual memory, thus invalidating
    //   the stack pointer to the struct.
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
            .write(libstd::registers::CR3::read().0.as_usize() as u32);

        libstd::memory::malloc::set(&KERNEL_MALLOC);

        debug!("Flagging `text` and `rodata` kernel sections as read-only.");
        let malloc = libstd::memory::malloc::get();
        for page in (__text_start.as_page()..__text_end.as_page())
            .chain(__rodata_start.as_page()..__rodata_end.as_page())
        {
            malloc.set_page_attributes(
                &page,
                libstd::memory::paging::PageAttributes::WRITABLE,
                libstd::memory::paging::AttributeModify::Remove,
            );
        }

        debug!("Memory initialization complete.");
    }

    _startup()
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    libstd::structures::gdt::init();
    libstd::structures::idt::load();
    libstd::cpu::init();
    init_apic();

    // If this is the BSP, wake other cores.
    if libstd::cpu::is_bsp() {
        use libstd::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        info!("Beginning wake-up sequence for each enabled processor core.");
        let apic = libstd::cpu::get().apic();
        let icr = apic.interrupt_command_register();
        if let Ok(madt) = XSDT.find_sub_table::<MADT>() {
            for interrupt_device in madt.iter() {
                if let InterruptDevice::LocalAPIC(lapic_other) = interrupt_device {
                    use libstd::acpi::rdsp::xsdt::madt::LocalAPICFlags;

                    // Ensure the CPU core can actually be enabled.
                    if lapic_other.flags().intersects(
                        LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                    ) && apic.id() != lapic_other.id()
                    {
                        const STACK_SIZE: usize = 1000000 /* 1 MiB */;
                        unsafe {
                            AP_STACK_POINTERS[lapic_other.id() as usize] =
                                (libstd::alloc!(STACK_SIZE) as *mut u8).add(STACK_SIZE) as usize;
                        }

                        let ap_trampoline_page_index =
                            unsafe { __ap_trampoline_start.as_page().index() } as u8;

                        icr.send_init(lapic_other.id());
                        icr.wait_pending();

                        icr.send_sipi(ap_trampoline_page_index, lapic_other.id());
                        icr.wait_pending();
                        icr.send_sipi(ap_trampoline_page_index, lapic_other.id());
                        icr.wait_pending();
                    }
                }
            }
        }
    }

    if libstd::cpu::is_bsp() {
        info!("Kernel has reached safe shutdown state.");
        unsafe { libstd::instructions::pwm::qemu_shutdown() }
    } else {
        libstd::instructions::hlt_indefinite()
    }
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
        libstd::memory::to_mibibytes(total_phys_memory),
        total_phys_memory
    );

    let frame_alloc_frame_count =
        falloc::FrameAllocator::frame_count_hint(total_falloc_memory) as u64;
    let frame_alloc_ptr = memory_map
        .iter()
        .filter(|descriptor| descriptor.ty == libstd::memory::UEFIMemoryType::CONVENTIONAL)
        .find(|descriptor| descriptor.page_count >= frame_alloc_frame_count)
        .map(|descriptor| descriptor.phys_start.as_usize() as *mut _)
        .expect("failed to find viable memory descriptor for memory map");

    falloc::load(frame_alloc_ptr, total_falloc_memory);
    debug!("Kernel frame allocator initialized.");
}

fn reserve_kernel_stack(memory_map: &[UEFIMemoryDescriptor]) -> libstd::memory::FrameIterator {
    debug!("Allocating frames according to BIOS memory map.");

    let mut last_frame_end = 0;
    let mut stack_frames = core::lazy::OnceCell::<libstd::memory::FrameIterator>::new();
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
        libstd::acpi::init_system_config_table(config_table_ptr, config_table_entry_len);

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
    use libstd::structures::idt;

    let apic = &libstd::cpu::get().apic();

    apic.auto_configure_timer_frequency();

    idt::set_interrupt_handler(32, timer::apic_tick_handler);
    apic.timer().set_vector(32);
    idt::set_interrupt_handler(58, apic_error_handler);
    apic.error().set_vector(58);

    apic.timer()
        .set_mode(libstd::structures::apic::TimerMode::Periodic);
    apic.timer().set_masked(false);
    apic.sw_enable();

    info!("Core-local APIC configured and enabled.");
}

extern "x86-interrupt" fn apic_error_handler(_: libstd::structures::idt::InterruptStackFrame) {
    let apic = &libstd::cpu::get().apic();

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}
