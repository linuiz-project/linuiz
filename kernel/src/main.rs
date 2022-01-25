#![no_std]
#![no_main]
#![feature(
    abi_efiapi,
    abi_x86_interrupt,
    once_cell,
    const_mut_refs,
    raw_ref_op,
    const_option_ext,
    naked_functions,
    asm_sym,
    asm_const
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libstd;

mod block_malloc;
mod clock;
mod drivers;
mod local_state;
mod logging;
mod scheduling;

use libstd::{
    acpi::SystemConfigTableEntry,
    cell::SyncOnceCell,
    memory::{falloc, malloc::MemoryAllocator, UEFIMemoryDescriptor},
    BootInfo, LinkerSymbol,
};

extern "C" {
    static __ap_trampoline_start: LinkerSymbol;
    static __ap_trampoline_end: LinkerSymbol;
    static __kernel_pml4: LinkerSymbol;

    static __gdt: LinkerSymbol;
    #[link_name = "__gdt.pointer"]
    static __gdt_pointer: LinkerSymbol;
    #[link_name = "__gdt.kcode"]
    static __gdt_kcode: LinkerSymbol;
    #[link_name = "__gdt.kdata"]
    static __gdt_kdata: LinkerSymbol;
    #[link_name = "__gdt.ucode"]
    static __gdt_ucode: LinkerSymbol;
    #[link_name = "__gdt.udata"]
    static __gdt_udata: LinkerSymbol;
    #[link_name = "__gdt.tss"]
    static __gdt_tss: LinkerSymbol;

    static __bsp_stack_bottom: LinkerSymbol;
    static __bsp_stack_top: LinkerSymbol;

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
static mut AP_STACK_POINTERS: [*const (); 256] = [core::ptr::null(); 256];

fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);
static BOOT_INFO: SyncOnceCell<BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>> =
    SyncOnceCell::new();
static KERNEL_MALLOCATOR: SyncOnceCell<block_malloc::BlockAllocator> = SyncOnceCell::new();

/// Clears the kernel stack by resetting `RSP`.
///
/// SAFETY: This method does *extreme* damage to the stack. It should only ever be used when
///         ABSOLUTELY NO dangling references to the old stack will exist (i.e. calling a
///         no-argument non-returning function directly after).
macro_rules! clear_bsp_stack {
    () => {
        assert!(
            $crate::local_state::is_bsp(),
            "Cannot clear AP stack pointers to BSP stack top."
        );

        libstd::registers::stack::RSP::write(__bsp_stack_top.as_ptr());
        // Serializing instruction to clear pipeline of any dangling references (and order all instruction before / after).
        libstd::instructions::cpuid::exec(0x0, 0x0).unwrap();
    };
}

#[no_mangle]
#[export_name = "_entry"]
unsafe extern "efiapi" fn kernel_init(
    boot_info: BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    /* PRE-INIT (no environment prepared) */
    if let Err(_) = BOOT_INFO.set(boot_info) {
        libstd::instructions::interrupts::breakpoint();
    }

    clear_bsp_stack!();

    /* INIT STDOUT */
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);

    match drivers::stdout::set_stdout(&mut CON_OUT, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => libstd::instructions::interrupts::breakpoint(),
    }

    /* INIT GDT */
    __gdt
        .as_mut_ptr::<u8>()
        .add(__gdt_tss.as_usize())
        .cast::<libstd::structures::gdt::TSSEntry>()
        .write_volatile(libstd::structures::gdt::TSS.as_gdt_entry());
    libstd::instructions::segmentation::lgdt(
        __gdt_pointer
            .as_ptr::<libstd::structures::gdt::DescriptorTablePointer>()
            .as_ref()
            .unwrap(),
    );
    libstd::instructions::init_segment_registers(__gdt_kdata.as_usize() as u16);
    use x86_64::instructions::segmentation::Segment;
    x86_64::instructions::segmentation::CS::set_reg(core::mem::transmute(
        __gdt_kcode.as_usize() as u16
    ));
    x86_64::instructions::tables::load_tss(x86_64::registers::segmentation::SegmentSelector(
        __gdt_tss.as_u64() as u16,
    ));

    // Brace execution of this block, to avoid accidentally using `boot_info` after stack is cleared.
    {
        let boot_info = BOOT_INFO
            .get()
            .expect("Boot info hasn't been initialized in kernel memory");

        info!("Validating BootInfo struct.");
        boot_info.validate_magic();

        debug!(
            "CPU features: {:?} | {:?}",
            libstd::instructions::cpuid::FEATURES,
            libstd::instructions::cpuid::FEATURES_EXT
        );

        /* INIT FRAME ALLOCATOR */
        debug!("Initializing kernel frame allocator.");
        falloc::load_new(boot_info.memory_map());

        /* INIT SYSTEM CONFIGURATION TABLE */
        info!("Initializing system configuration table.");
        let config_table_ptr = boot_info.config_table().as_ptr();
        let config_table_entry_len = boot_info.config_table().len();
        let frame_index = libstd::align_down_div(config_table_ptr as usize, 0x1000);
        let frame_count = libstd::align_down_div(
            config_table_entry_len * core::mem::size_of::<SystemConfigTableEntry>(),
            0x1000,
        );
        // Assign system configuration table prior to reserving frames to ensure one doesn't already exist.
        libstd::acpi::init_system_config_table(config_table_ptr, config_table_entry_len);
        let falloc = falloc::get();
        for frame_index in frame_index..(frame_index + frame_count) {
            falloc.borrow(frame_index).unwrap();
        }
    }

    clear_bsp_stack!();

    /* INIT KERNEL MEMORY */
    {
        info!("Initializing kernel default allocator.");

        let malloc = block_malloc::BlockAllocator::new();
        debug!("Flagging `text` and `rodata` kernel sections as read-only.");
        use libstd::memory::Page;
        let text_page_range = Page::from_index(__text_start.as_usize() / 0x1000)
            ..=Page::from_index(__text_end.as_usize() / 0x1000);
        let rodata_page_range = Page::from_index(__rodata_start.as_usize() / 0x1000)
            ..=Page::from_index(__rodata_end.as_usize() / 0x1000);
        for page in text_page_range.chain(rodata_page_range) {
            malloc.set_page_attribs(
                &page,
                libstd::memory::paging::PageAttributes::WRITABLE,
                libstd::memory::paging::AttributeModify::Remove,
            );
        }

        debug!("Setting libstd's default memory allocator to new kernel allocator.");
        KERNEL_MALLOCATOR.set(malloc).map_err(|_| panic!()).ok();
        libstd::memory::malloc::set(KERNEL_MALLOCATOR.get().unwrap());
        // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
        debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
        __kernel_pml4
            .as_mut_ptr::<u32>()
            .write(libstd::registers::CR3::read().0.as_usize() as u32);

        info!("Kernel memory initialized.");
    }

    clear_bsp_stack!();

    /* COMMON KERNEL START (prepare local state and AP processors) */
    _startup()
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    // Ensure we load the IDT as early as possible in startup sequence.
    unsafe { libstd::structures::idt::load_unchecked() };

    if crate::local_state::is_bsp() {
        use libstd::structures::idt;
        use local_state::{handlers, InterruptVector};

        // This is where we'll configure the kernel-static IDT entries.
        idt::set_handler_fn(InterruptVector::LocalTimer as u8, handlers::apit_handler);
        idt::set_handler_fn(InterruptVector::Storage as u8, handlers::storage_handler);
        idt::set_handler_fn(InterruptVector::Spurious as u8, handlers::spurious_handler);
        idt::set_handler_fn(InterruptVector::Error as u8, handlers::error_handler);

        // Initialize global clock (PIT).
        // TODO possible move to using HPET as global clock?
        crate::clock::global::init();
    }

    // Initialize the processor-local state.
    crate::local_state::init();

    // If this is the BSP, wake other cores.
    if crate::local_state::is_bsp() {
        use libstd::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        let id = crate::local_state::processor_id();
        let icr = crate::local_state::int_ctrl().icr();
        let ap_trampoline_page_index = unsafe { __ap_trampoline_start.as_usize() / 0x1000 } as u8;

        if let Ok(madt) = XSDT.find_sub_table::<MADT>() {
            info!("Beginning wake-up sequence for enabled processors.");
            for lapic in madt
                .iter()
                // Filter out non-lapic devices.
                .filter_map(|interrupt_device| {
                    if let InterruptDevice::LocalAPIC(apic_other) = interrupt_device {
                        Some(apic_other)
                    } else {
                        None
                    }
                })
                // Filter out invalid lapic devices.
                .filter(|lapic| {
                    use libstd::acpi::rdsp::xsdt::madt::LocalAPICFlags;

                    lapic.id() != id
                        && lapic.flags().intersects(
                            LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                        )
                })
            {
                unsafe {
                    const AP_STACK_SIZE: usize = 0x2000;

                    let (stack_bottom, len) = libstd::memory::malloc::try_get()
                        .unwrap()
                        .alloc(AP_STACK_SIZE, core::num::NonZeroUsize::new(0x1000))
                        .unwrap()
                        .into_parts();

                    AP_STACK_POINTERS[lapic.id() as usize] = stack_bottom.add(len) as *mut _;
                };

                // Reset target processor.
                trace!("Sending INIT interrupt to: {}", lapic.id());
                icr.send_init(lapic.id());
                icr.wait_pending();
                // REMARK: IA32 spec indicates that doing this twice, as so, ensures the interrupt is received.
                trace!("Sending SIPI x1 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_trampoline_page_index, lapic.id());
                icr.wait_pending();
                trace!("Sending SIPI x2 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_trampoline_page_index, lapic.id());
                icr.wait_pending();
            }
        }

        //     use libstd::io::pci;
        //     let nvme_driver = pci::BRIDGES
        //         .lock()
        //         .iter()
        //         .flat_map(|bridge| bridge.iter())
        //         .flat_map(|bus| bus.iter())
        //         .find_map(|device| {
        //             if let pci::DeviceVariant::Standard(device) = device_variant {
        //                 if device.class() == pci::DeviceClass::MassStorageController
        //                     && device.subclass() == 0x08
        //                 {
        //                     Some(drivers::nvme::Controller::from_device(&device, 4, 4))
        //                 }
        //             }
        //         });

        //     use scheduling::Task;
        //     let mut thread = local_state::lock_thread();
        //     let mut nvme = nvme_driver.expect("No NVMe drive detected.");
        //     thread.push_task(Task::new(128, nvme.run, None, None));
        //     thread.push_task(Task::new(0, bsp_main, None, None));
        //     thread.set_enabled(true);
        // }

        use scheduling::Task;
        let mut thread = local_state::lock_thread();
        //thread.set_enabled(true);
        thread.push_task(Task::new(255, task1, None, None));
        thread.push_task(Task::new(255, task2, None, None));
    }
    // libstd::instructions::hlt_indefinite()

    unsafe {
        use libstd::registers::MSR;

        // Enable `sysenter`/`sysexit`.
        MSR::IA32_EFER.write(MSR::IA32_EFER.read() | 0x1);
        // Configure system call environment registers.
        MSR::IA32_STAR.write((__gdt_kdata.as_u64() << 48) | (__gdt_kcode.as_u64() << 32));
        MSR::IA32_SFMASK.write(u64::MAX);
        // MSR::IA32_LSTAR.write(0x0);
    }

    kernel_main()
}

#[naked]
unsafe fn swap_ring3() {
    core::arch::asm!(
    "
        lea rcx, {}
        mov r11, {}

        sysretq
    ",
    sym test_user_function,
    const libstd::registers::RFlags::INTERRUPT_FLAG.bits(),
    options(noreturn)
    );
}

fn test_user_function() {
    libstd::instructions::hlt_indefinite();
}

fn task1() -> ! {
    loop {
        for i in 65..91 {
            unsafe { CON_OUT.write(i) };
            clock::global::busy_wait_msec(1);
        }
    }
}

fn task2() -> ! {
    loop {
        for i in 97..123 {
            unsafe { CON_OUT.write(i) };
            clock::global::busy_wait_msec(1);
        }
    }
}

fn bsp_main(nvme: drivers::nvme::Controller) -> ! {
    libstd::instructions::hlt_indefinite()
}

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    unsafe { swap_ring3() };

    libstd::instructions::hlt_indefinite()
}
