#![no_std]
#![no_main]
#![feature(abi_efiapi, abi_x86_interrupt, once_cell, const_mut_refs, raw_ref_op)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libstd;

mod block_malloc;
mod clock;
mod drivers;
mod local_state;
mod logging;

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
    #[link_name = "__gdt.pointer"]
    static __gdt_pointer: LinkerSymbol;
    #[link_name = "__gdt.code"]
    static __gdt_code: LinkerSymbol;
    #[link_name = "__gdt.data"]
    static __gdt_data: LinkerSymbol;

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
#[inline(always)]
unsafe fn clear_bsp_stack() {
    assert!(
        crate::local_state::is_bsp(),
        "Cannot clear AP stack pointers to BSP stack bottom."
    );

    libstd::registers::stack::RSP::write(__bsp_stack_top.as_ptr());
    // Serializing instruction to clear pipeline of any dangling references.
    libstd::instructions::cpuid::exec(0x0, 0x0).unwrap();
}

#[no_mangle]
#[export_name = "_entry"]
unsafe extern "efiapi" fn _kernel_pre_init(
    boot_info: BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    if let Err(_) = BOOT_INFO.set(boot_info) {
        libstd::instructions::interrupts::breakpoint();
    }

    clear_bsp_stack();
    kernel_init()
}

unsafe fn kernel_init() -> ! {
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);

    match drivers::stdout::set_stdout(&mut CON_OUT, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => libstd::instructions::interrupts::breakpoint(),
    }

    {
        libstd::instructions::segmentation::lgdt(
            __gdt_pointer
                .as_ptr::<libstd::structures::gdt::DescriptorTablePointer>()
                .as_ref()
                .unwrap(),
        );
        libstd::instructions::init_segment_registers(__gdt_data.as_usize() as u16);
        use x86_64::instructions::segmentation::Segment;
        x86_64::instructions::segmentation::CS::set_reg(core::mem::transmute(
            __gdt_code.as_usize() as u16,
        ));
    }

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

    debug!("Initializing kernel frame allocator.");
    falloc::load_new(boot_info.memory_map());
    init_system_config_table(boot_info.config_table());

    clear_bsp_stack();
    kernel_mem_init()
}

unsafe fn kernel_mem_init() -> ! {
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

    info!("Finalizing kernel memory allocator initialization.");

    clear_bsp_stack();
    _startup()
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    use crate::local_state::is_bsp;

    // Ensure we load the IDT as early as possible in startup sequence.
    unsafe { libstd::structures::idt::load_unchecked() };

    if is_bsp() {
        use libstd::structures::idt;
        use local_state::{handlers, InterruptVector};

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
    if is_bsp() {
        use libstd::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        let id = crate::local_state::processor_id();
        let icr = crate::local_state::int_ctrl().unwrap().icr();
        let ap_trampoline_page_index = unsafe { __ap_trampoline_start.as_page().index() } as u8;

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
                // REMARK: IA32 spec indicates doing it twice, as so, ensures the interrupt is received.
                trace!("Sending SIPI x1 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_trampoline_page_index, lapic.id());
                icr.wait_pending();
                trace!("Sending SIPI x2 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_trampoline_page_index, lapic.id());
                icr.wait_pending();
            }
        }
    }

    kernel_main()
}

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    if crate::local_state::is_bsp() {
        use libstd::{
            acpi::rdsp::xsdt::{mcfg::MCFG, XSDT},
            io::pci,
        };

        if let Ok(mcfg) = XSDT.find_sub_table::<MCFG>() {
            let bridges: alloc::vec::Vec<pci::PCIeHostBridge> = mcfg
                .iter()
                .filter_map(|entry| pci::configure_host_bridge(entry).ok())
                .collect();

            for device_variant in bridges
                .iter()
                .flat_map(|bridge| bridge.iter())
                .flat_map(|bus| bus.iter())
            {
                if let pci::DeviceVariant::Standard(device) = device_variant {
                    if device.class() == pci::DeviceClass::MassStorageController
                        && device.subclass() == 0x08
                    {
                        use crate::drivers::nvme::{command::Command, Controller};
                        let mut nvme = Controller::from_device(device, 4, 4);

                        let sub_queue = nvme.admin_submission_queue();
                        let (data, command) = Command::identify(0);
                        sub_queue.submit_command(command).unwrap();
                        sub_queue.flush_commands();

                        crate::clock::local::sleep_msec(10);

                        info!("{:?}", data);
                    }
                }
            }
        }
    }

    libstd::instructions::hlt_indefinite()
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
        let falloc = falloc::get();
        for frame_index in frame_index..(frame_index + frame_count) {
            falloc.borrow(frame_index).unwrap();
        }
    }
}
