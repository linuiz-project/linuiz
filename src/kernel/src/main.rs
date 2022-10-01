#![no_std]
#![no_main]
#![feature(
    asm_const,
    asm_sym,
    naked_functions,
    abi_x86_interrupt,
    sync_unsafe_cell,
    panic_info_message,
    allocator_api,
    once_cell,
    pointer_is_aligned,
    slice_ptr_get,
    strict_provenance,
    core_intrinsics,
    alloc_error_handler,
    exclusive_range_pattern,
    raw_ref_op,
    let_chains,
    unchecked_math,
    cstr_from_bytes_until_nul,
    if_let_guard,
    inline_const,
    exact_size_is_empty
)]
#![forbid(clippy::inline_asm_x86_att_syntax)]
#![deny(clippy::semicolon_if_nothing_returned, clippy::debug_assert_with_mut_call, clippy::float_arithmetic)]
#![warn(clippy::cargo, clippy::pedantic, clippy::undocumented_unsafe_blocks)]
#![allow(
    clippy::cast_lossless,
    clippy::enum_glob_use,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::must_use_candidate,
    clippy::unreadable_literal,
    clippy::wildcard_imports,
    dead_code
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libcommon;

mod acpi;
mod elf;
mod local_state;
mod memory;
mod num;
mod panic;
mod scheduling;
mod stdout;
mod syscall;
mod time;

use core::{num::NonZeroUsize, sync::atomic::Ordering};
use libcommon::{Address, Frame, Page, Virtual};

pub const LIMINE_REV: u64 = 0;
static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);
static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV).flags(0b1);
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);
static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(LIMINE_REV);
static LIMINE_STACK: limine::LimineStackSizeRequest = limine::LimineStackSizeRequest::new(LIMINE_REV).stack_size({
    #[cfg(debug_assertions)]
    {
        0x1000000
    }

    #[cfg(not(debug_assertions))]
    {
        0x4000
    }
});

static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static SMP_MEMORY_INIT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

// TODO parse kernel command line configuration more succintly
static mut KERNEL_CFG_SMP: bool = false;

pub type MmapEntry = limine::NonNullPtr<limine::LimineMemmapEntry>;
pub type MmapEntryType = limine::LimineMemoryMapEntryType;

/// SAFETY: Do not call this function in software.
#[no_mangle]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    /* standard output setup */
    {
        use crate::memory::io::SerialWriter;
        use core::cell::SyncUnsafeCell;
        use spin::Lazy;

        static UART: Lazy<SyncUnsafeCell<SerialWriter>> = Lazy::new(|| {
            SyncUnsafeCell::new({
                // SAFETY: Function is called only this once, statically (lazily).
                unsafe { SerialWriter::init() }
            })
        });

        crate::stdout::set_stdout(&mut *UART.get(), log::LevelFilter::Trace);
    }

    info!("Successfully loaded into kernel.");

    /* info dump */
    {
        let boot_info = LIMINE_INFO.get_response().get().expect("bootloader provided no info");
        info!(
            "Bootloader Info     {} v{} (rev {})",
            core::ffi::CStr::from_ptr(boot_info.name.as_ptr().unwrap().cast()).to_str().unwrap(),
            core::ffi::CStr::from_ptr(boot_info.version.as_ptr().unwrap().cast()).to_str().unwrap(),
            boot_info.revision,
        );

        #[cfg(target_arch = "x86_64")]
        if let Some(vendor_str) = libarch::x64::cpu::get_vendor() {
            info!("Vendor              {vendor_str}");
        } else {
            info!("Vendor              None");
        }
    }

    // TODO do this somewhere that makes sense
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: Provided IRQ base is intentionally within the exception range for x86 CPUs.
        static PICS: spin::Mutex<pic_8259::Pics> = spin::Mutex::new(unsafe { pic_8259::Pics::new(0) });
        PICS.lock().init(pic_8259::InterruptLines::empty());
    }

    /* set interrupt handlers */
    {
        libarch::interrupts::set_page_fault_handler({
            use libarch::interrupts::PageFaultHandlerError;

            /// SAFETY: This function expects only to be called upon a processor page fault exception.
            unsafe fn pf_handler(address: Address<Virtual>) -> Result<(), PageFaultHandlerError> {
                use crate::memory::PageAttributes;

                let fault_page = Address::<Page>::new(address, None).unwrap();
                let virtual_mapper =
                    crate::memory::VirtualMapper::from_current(crate::memory::get_kernel_hhdm_address());
                let Some(mut fault_page_attributes) = virtual_mapper.get_page_attributes(fault_page) else { return Err(PageFaultHandlerError::AddressNotMapped) };
                if fault_page_attributes.contains(PageAttributes::DEMAND) {
                    virtual_mapper
                        .auto_map(fault_page, {
                            // remove demand bit ...
                            fault_page_attributes.remove(PageAttributes::DEMAND);
                            // ... insert present bit ...
                            fault_page_attributes.insert(PageAttributes::PRESENT);
                            // ... return attributes
                            fault_page_attributes
                        })
                        .unwrap();

                    // SAFETY: We know the page was just mapped, and contains no relevant memory.
                    fault_page.zero_memory();

                    Ok(())
                } else {
                    Err(PageFaultHandlerError::NotDemandPaged)
                }
            }

            pf_handler
        });

        libarch::interrupts::set_interrupt_handler({
            use libarch::interrupts::{ArchContext, ControlFlowContext, Vector};

            fn common_interrupt_handler(
                irq_vector: u64,
                ctrl_flow_context: &mut ControlFlowContext,
                arch_context: &mut ArchContext,
            ) {
                match Vector::try_from(irq_vector) {
                    Ok(vector) if vector == Vector::Timer => {
                        crate::local_state::schedule_next_task(ctrl_flow_context, arch_context);
                    }

                    vector_result => {
                        warn!("Unhandled IRQ vector: {:?}", vector_result);
                    }
                }

                #[cfg(target_arch = "x86_64")]
                libarch::x64::structures::apic::end_of_interrupt();
            }

            common_interrupt_handler
        });

        libarch::interrupts::set_syscall_handler(crate::syscall::syscall_handler)
    }

    /* parse kernel file */
    // TODO parse the kernel file in a module or something
    {
        let kernel_file = LIMINE_KERNEL_FILE
            .get_response()
            .get()
            .expect("bootloader did not provide a kernel file")
            .kernel_file
            .get()
            .expect("bootloader kernel file response did not provide a valid file handle");
        let cmdline = kernel_file.cmdline.to_str().unwrap().to_str().expect("invalid cmdline string");

        for argument in cmdline.split(' ') {
            match argument.split_once(':') {
                Some(("smp", "on")) => KERNEL_CFG_SMP = true,
                Some(("smp", "off")) => KERNEL_CFG_SMP = false,
                _ => warn!("Unhandled cmdline parameter: {:?}", argument),
            }
        }

        let kernel_bytes = core::slice::from_raw_parts(kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize);
        let kernel_elf = crate::elf::Elf::from_bytes(&kernel_bytes).expect("failed to parse kernel executable");
        if let Some(names_section) = kernel_elf.get_section_names_section() {
            for section in kernel_elf.iter_sections() {
                if let Some(section_name) =
                    core::ffi::CStr::from_bytes_until_nul(&names_section.data()[section.get_names_section_offset()..])
                        .ok()
                        .and_then(|cstr_name| cstr_name.to_str().ok())
                {
                    match section_name {
                        ".symtab" => {
                            panic::KERNEL_SYMBOLS.call_once(|| {
                                let (pre, symbols, post) = section.data().align_to::<crate::elf::symbol::Symbol>();

                                // Ensure the symbols are properly aligned to safely convert.
                                debug_assert!(pre.is_empty());
                                debug_assert!(post.is_empty());

                                core::slice::from_raw_parts(symbols.as_ptr(), symbols.len())
                            });
                        }

                        ".strtab" => {
                            panic::KERNEL_STRINGS.call_once(|| {
                                let data = section.data();
                                core::slice::from_raw_parts(data.as_ptr(), data.len())
                            });
                        }

                        _ => {}
                    }
                }
            }
        }
    }

    crate::memory::init_kernel_hhdm_address();
    crate::memory::init_global_allocator(LIMINE_MMAP.get_response().get().unwrap().memmap());
    crate::memory::init_kernel_page_manager();

    /* bsp core init */
    {
        #[cfg(target_arch = "x86_64")]
        libarch::x64::cpu::init();

        // TODO rv64 bsp hart init
    }

    let to_mapper = crate::memory::get_kernel_virtual_mapper();

    /* memory init */
    {
        use crate::memory::PageAttributes;
        use crate::memory::VirtualMapper;
        use libcommon::{LinkerSymbol, PageAlign};

        extern "C" {
            static __text_start: LinkerSymbol;
            static __text_end: LinkerSymbol;

            static __rodata_start: LinkerSymbol;
            static __rodata_end: LinkerSymbol;

            static __bss_start: LinkerSymbol;
            static __bss_end: LinkerSymbol;

            static __data_start: LinkerSymbol;
            static __data_end: LinkerSymbol;
        }

        debug!("Initializing kernel page manager...");

        let hhdm_address = crate::memory::get_kernel_hhdm_address();
        let from_mapper = VirtualMapper::from_current(hhdm_address);

        {
            fn map_range_from(
                from_mapper: &VirtualMapper,
                to_mapper: &VirtualMapper,
                range: core::ops::Range<u64>,
                attributes: PageAttributes,
            ) {
                // SAFETY: Linker should have correctly set this value.
                let page_align = unsafe {
                    extern "C" {
                        static __SECTION_ALIGN: LinkerSymbol;
                    }

                    PageAlign::from_u64(__SECTION_ALIGN.as_u64()).unwrap()
                };

                for address in range.step_by(page_align.as_usize()).map(Address::<Virtual>::new_truncate) {
                    to_mapper
                        .map(
                            Address::<Page>::new_truncate(address, page_align),
                            // Properly handle the bootloader possibly not using huge page mappings (but still physically contiguous).
                            Address::<Page>::new(address, None)
                                .and_then(|page_address| from_mapper.get_mapped_to(page_address))
                                .unwrap(),
                            false,
                            attributes,
                        )
                        .unwrap();
                }
            }

            map_range_from(
                &from_mapper,
                &to_mapper,
                __text_start.as_u64()..__text_end.as_u64(),
                PageAttributes::RX | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __rodata_start.as_u64()..__rodata_end.as_u64(),
                PageAttributes::RO | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __bss_start.as_u64()..__bss_end.as_u64(),
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
            map_range_from(
                &from_mapper,
                &to_mapper,
                __data_start.as_u64()..__data_end.as_u64(),
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
        }

        for entry in LIMINE_MMAP.get_response().get().unwrap().memmap() {
            let page_attributes = {
                use limine::LimineMemoryMapEntryType;
                match entry.typ {
                    LimineMemoryMapEntryType::Usable
                    | LimineMemoryMapEntryType::AcpiNvs
                    | LimineMemoryMapEntryType::AcpiReclaimable
                    | LimineMemoryMapEntryType::BootloaderReclaimable
                    // TODO handle the PATs or something to make this WC
                    | LimineMemoryMapEntryType::Framebuffer => PageAttributes::RW,

                    LimineMemoryMapEntryType::Reserved | LimineMemoryMapEntryType::KernelAndModules => {
                        PageAttributes::RO
                    }

                    LimineMemoryMapEntryType::BadMemory => continue,
                }
            };

            for phys_base in (entry.base..(entry.base + entry.len)).step_by(0x1000) {
                // TODO use huge pages here if possible
                to_mapper
                    .map(
                        Address::<Page>::new_truncate(
                            Address::<Virtual>::new_truncate(hhdm_address.as_u64() + phys_base),
                            PageAlign::Align4KiB,
                        ),
                        Address::<Frame>::new_truncate(phys_base),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }
        }
    }

    /* SMP init */
    // Because the SMP information structures (and thus, their `goto_address`) are only mapped in the bootloader
    // page tables, we have to start the other cores and pass the root page table frame index in. All of the cores
    // will then wait until every core has swapped to the new page tables, then this core (the boot core) will
    // reclaim bootloader memory.
    {
        debug!("Attempting to start additional cores...");

        let smp_response = LIMINE_SMP.get_response().get_mut().expect("bootloader provided no SMP information");
        debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
        let bsp_lapic_id = smp_response.bsp_lapic_id;

        for cpu_info in smp_response.cpus() {
            if cpu_info.lapic_id != bsp_lapic_id {
                if KERNEL_CFG_SMP {
                    trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

                    SMP_MEMORY_INIT.fetch_add(1, Ordering::Relaxed);
                    cpu_info.goto_address = {
                        // REMARK: Function is placed locally here to ensure it is never called in another context.
                        extern "C" fn _smp_entry(_: *const limine::LimineSmpInfo) -> ! {
                            // Wait to ensure the machine is the correct state to execute cpu setup.
                            while !SMP_MEMORY_READY.load(Ordering::Relaxed) {
                                core::hint::spin_loop();
                            }

                            // SAFETY: Function is called only once per core.
                            #[cfg(target_arch = "x86_64")]
                            unsafe {
                                libarch::x64::cpu::init()
                            };

                            // SAFETY: All currently referenced memory should be mapped in the kernel page tables.
                            unsafe { crate::memory::get_kernel_virtual_mapper().commit_vmem_register().unwrap() };

                            SMP_MEMORY_INIT.fetch_sub(1, Ordering::Relaxed);
                            while SMP_MEMORY_INIT.load(Ordering::Relaxed) > 0 {
                                core::hint::spin_loop();
                            }

                            trace!("Finished SMP entry for core.");

                            // SAFETY: Function is called only once.
                            unsafe { kernel_thread_setup() }
                        }

                        _smp_entry
                    };
                } else {
                    cpu_info.goto_address = {
                        extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                            // SAFETY: Core is not expecting anything to happen, as it will be idling.
                            unsafe { libarch::interrupts::disable() };
                            libarch::interrupts::wait_indefinite()
                        }

                        _idle_forever
                    };
                }
            }
        }
    }

    /* memory finalize */
    {
        debug!("Switching to kernel page tables...");
        to_mapper.commit_vmem_register().unwrap();
        debug!("Kernel has finalized control of page tables.");

        #[cfg(target_arch = "x86_64")]
        {
            debug!("Initializing APIC interface...");
            libarch::x64::structures::apic::init_apic(|address| {
                let page_address = Address::<Page>::new(
                    Address::<Virtual>::new(crate::memory::get_kernel_hhdm_address().as_u64() + (address as u64))
                        .unwrap(),
                    Some(libcommon::PageAlign::Align4KiB),
                )
                .unwrap();

                crate::memory::get_kernel_virtual_mapper()
                    .map_if_not_mapped(
                        page_address,
                        Some((Address::<libcommon::Frame>::new(address as u64).unwrap(), false)),
                        crate::memory::PageAttributes::MMIO,
                    )
                    .unwrap();

                page_address.address().as_mut_ptr()
            })
            .unwrap();
        }

        debug!("Initializing ACPI interface...");
        {
            static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(crate::LIMINE_REV);

            let rsdp_address = LIMINE_RSDP
                .get_response()
                .get()
                .expect("bootloader provided no RSDP address")
                .address
                .as_ptr()
                .unwrap()
                .addr();
            let hhdm_address = crate::memory::get_kernel_hhdm_address().as_usize();
            crate::acpi::init_interface(libcommon::Address::<libcommon::Physical>::new_truncate(
                // Properly handle the bootloader's mapping of ACPI addresses in lower-half or higher-half memory space.
                if rsdp_address > hhdm_address { rsdp_address - hhdm_address } else { rsdp_address } as u64,
            ));
        }

        debug!("Loading pre-packaged drivers...");
        // load_drivers();

        // TODO init PCI devices
        // debug!("Initializing PCI devices...");
        // crate::memory::io::pci::init_devices();

        debug!("Boot core will release other cores, and then wait for all cores to update root page table.");
        SMP_MEMORY_READY.store(true, Ordering::Relaxed);

        while SMP_MEMORY_INIT.load(Ordering::Relaxed) > 0 {
            core::hint::spin_loop();
        }

        // TODO reclaim bootloader memory
        // debug!("Reclaiming bootloader reclaimable memory...");
        // crate::memory::reclaim_bootloader_frames();
    }

    /* configure I/O APIC redirections */
    // #[cfg(target_arch = "x86_64")]
    // {
    //     debug!("Configuring I/O APIC and processing interrupt overrides.");

    //     let ioapics = libarch::x64::structures::ioapic::get_io_apics();
    //     let platform_info = crate::acpi::get_platform_info();

    //     if let acpi::platform::interrupt::InterruptModel::Apic(apic) = &platform_info.interrupt_model {
    //         use libarch::interrupts;

    //         let mut cur_vector = 0x70;

    //         for irq_source in apic.interrupt_source_overrides.iter() {
    //             debug!("{:?}", irq_source);

    //             let target_ioapic = ioapics
    //                 .iter()
    //                 .find(|ioapic| ioapic.handled_irqs().contains(&irq_source.global_system_interrupt))
    //                 .expect("no I/I APIC found for IRQ override");

    //             let mut redirection = target_ioapic.get_redirection(irq_source.global_system_interrupt);
    //             redirection.set_delivery_mode(interrupts::DeliveryMode::Fixed);
    //             redirection.set_destination_mode(interrupts::DestinationMode::Logical);
    //             redirection.set_masked(false);
    //             redirection.set_pin_polarity(irq_source.polarity);
    //             redirection.set_trigger_mode(irq_source.trigger_mode);
    //             redirection.set_vector({
    //                 let vector = cur_vector;
    //                 cur_vector += 1;
    //                 vector
    //             });
    //             redirection.set_destination_id(0 /* TODO real cpu id */);

    //             debug!(
    //                 "IRQ override: Global {} -> {}:{}",
    //                 irq_source.global_system_interrupt,
    //                 redirection.get_destination_id(),
    //                 redirection.get_vector()
    //             );
    //             target_ioapic.set_redirection(irq_source.global_system_interrupt, &redirection);
    //         }

    //         for nmi_source in apic.nmi_sources.iter() {
    //             debug!("{:?}", nmi_source);

    //             let target_ioapic = ioapics
    //                 .iter()
    //                 .find(|ioapic| ioapic.handled_irqs().contains(&nmi_source.global_system_interrupt))
    //                 .expect("no I/I APIC found for IRQ override");

    //             let mut redirection = target_ioapic.get_redirection(nmi_source.global_system_interrupt);
    //             redirection.set_delivery_mode(interrupts::DeliveryMode::NMI);
    //             redirection.set_destination_mode(interrupts::DestinationMode::Logical);
    //             redirection.set_masked(false);
    //             redirection.set_pin_polarity(nmi_source.polarity);
    //             redirection.set_trigger_mode(nmi_source.trigger_mode);
    //             redirection.set_vector({
    //                 let vector = cur_vector;
    //                 cur_vector += 1;
    //                 vector
    //             });
    //             redirection.set_destination_id(0 /* TODO real cpu id */);

    //             debug!(
    //                 "NMI override: Global {} -> {}:{}",
    //                 nmi_source.global_system_interrupt,
    //                 redirection.get_destination_id(),
    //                 redirection.get_vector()
    //             );
    //             target_ioapic.set_redirection(nmi_source.global_system_interrupt, &redirection);
    //         }
    //     }

    //     // TODO ?? maybe
    //     // /* enable ACPI SCI interrupts */
    //     // {
    //     //     // TODO clean this filthy mess up

    //     //     let pm1a_evt_blk =
    //     //         &crate::tables::acpi::get_fadt().pm1a_event_block().expect("no `PM1a_EVT_BLK` found in FADT");

    //     //     let mut reg = libcommon::acpi::Register::<u16>::IO(crate::memory::io::ReadWritePort::new(
    //     //         (pm1a_evt_blk.address + ((pm1a_evt_blk.bit_width / 8) as u64)) as u16,
    //     //     ));

    //     //     reg.write((1 << 8) | (1 << 0));
    //     // }
    // }

    info!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, Ordering::Relaxed);

    // TODO make this a standalone function so we can return error states

    kernel_thread_setup()
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup() -> ! {
    crate::local_state::init(0);

    trace!("Beginning scheduling...");
    libarch::interrupts::enable();
    crate::local_state::begin_scheduling();

    trace!("Core will soon execute a task, or otherwise halt.");
    libarch::interrupts::wait_indefinite()
}

/* MODULE LOADING */
fn load_drivers() {
    let drivers_data = LIMINE_MODULES
        .get_response()
        .get()
        // Find the drives module, and map the `Option<>` to it.
        .and_then(|modules| {
            modules.modules().iter().find(|module| module.path.to_str().unwrap().to_str().unwrap().ends_with("drivers"))
        })
        // SAFETY: Kernel promises HHDM to be valid, and the module pointer should be in the HHDM, so this should be valid for `u8`.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");

    let mut current_offset = 0;
    while current_offset < drivers_data.len() {
        // Copy and reconstruct the driver byte length from the prefix.
        let driver_len = {
            let mut value = 0;

            value |= (drivers_data[current_offset + 0] as u64) << 0;
            value |= (drivers_data[current_offset + 1] as u64) << 8;
            value |= (drivers_data[current_offset + 2] as u64) << 16;
            value |= (drivers_data[current_offset + 3] as u64) << 24;
            value |= (drivers_data[current_offset + 4] as u64) << 32;
            value |= (drivers_data[current_offset + 5] as u64) << 40;
            value |= (drivers_data[current_offset + 6] as u64) << 48;
            value |= (drivers_data[current_offset + 7] as u64) << 56;

            value as usize
        };

        let base_offset = current_offset + 8 /* skip 'len' prefix */;
        let driver_data =
            miniz_oxide::inflate::decompress_to_vec(&drivers_data[base_offset..(base_offset + driver_len)])
                .expect("failed to decompress driver");
        let driver_elf = crate::elf::Elf::from_bytes(&driver_data).unwrap();
        info!("{:?}", driver_elf);

        load_driver(&driver_elf);

        current_offset += driver_len + 8  /* skip 'len' prefix */;
    }
}

fn load_driver(driver: &crate::elf::Elf) {
    use crate::{elf::segment, memory::PageAttributes};
    use libcommon::PageAlign;

    // Create the driver's page manager from the kernel's higher-half table.
    // SAFETY: Kernel guarantees HHDM to be valid.
    let driver_page_manager = unsafe {
        crate::memory::VirtualMapper::new(
            4,
            crate::memory::get_kernel_hhdm_address(),
            Some(libarch::memory::VmemRegister::read()),
        )
        .expect("failed to create page manager for driver")
    };

    let hhdm_address = crate::memory::get_kernel_hhdm_address();

    // Iterate the segments, and allocate them.
    for segment in driver.iter_segments() {
        trace!("{:?}", segment);

        match segment.get_type() {
            segment::Type::Loadable => {
                let memory_start = segment.get_virtual_address().unwrap().as_usize();
                let memory_end = memory_start + segment.get_memory_layout().unwrap().size();
                // SAFETY: Value provided is non-zero.
                let start_page_index =
                    libcommon::align_down_div(memory_start, unsafe { NonZeroUsize::new_unchecked(0x1000) });
                // SAFETY: Value provided is non-zero.
                let end_page_index =
                    libcommon::align_up_div(memory_end, unsafe { NonZeroUsize::new_unchecked(0x1000) });
                let mut data_offset = 0;

                for page_index in start_page_index..end_page_index {
                    // REMARK: This doesn't support RWX pages. I'm not sure it ever should.
                    let page_attributes = if segment.get_flags().contains(segment::Flags::EXECUTABLE) {
                        PageAttributes::RX
                    } else if segment.get_flags().contains(segment::Flags::WRITABLE) {
                        PageAttributes::RW
                    } else {
                        PageAttributes::RO
                    };

                    let page = Address::<Page>::new(
                        Address::<Virtual>::new((page_index * 0x1000) as u64).unwrap(),
                        Some(PageAlign::Align4KiB),
                    )
                    .unwrap();
                    driver_page_manager.auto_map(page, page_attributes | PageAttributes::USER).unwrap();

                    // SAFETY: HHDM is guaranteed by kernel to be valid, and the frame being pointed to was just allocated.
                    let memory_hhdm = unsafe {
                        core::slice::from_raw_parts_mut(
                            hhdm_address
                                .as_mut_ptr::<u8>()
                                .add(driver_page_manager.get_mapped_to(page).unwrap().as_usize()),
                            0x1000,
                        )
                    };

                    // If the virtual address isn't page-aligned, then this allows us to start writing at
                    // the correct address, rather than writing the wrong bytes at the lower page boundary.
                    let memory_offset = memory_start.checked_sub(page_index * 0x1000).unwrap_or(0);
                    // REMARK: This could likely be optimized to use memcpy / copy_nonoverlapping, but
                    //         for now this approach suffices.
                    for index in memory_offset..0x1000 {
                        let data_value = segment.data().get(data_offset);
                        memory_hhdm[index] = *data_value
                            // Handle zeroing of `.bss` segments.
                            .unwrap_or(&0);
                        data_offset += 1;
                    }
                }
            }

            _ => {}
        }
    }

    // Push ELF as global task.
    {
        let stack_address = {
            const TASK_STACK_BASE_ADDRESS: Address<Page> =
                Address::<Page>::new_truncate(Address::<Virtual>::new_truncate(128 << 39), PageAlign::Align2MiB);
            // TODO make this a dynamic configuration
            const TASK_STACK_PAGE_COUNT: usize = 2;

            for page in
                (0..TASK_STACK_PAGE_COUNT).map(|offset| TASK_STACK_BASE_ADDRESS.forward_checked(offset).unwrap())
            {
                driver_page_manager
                    .map(
                        page,
                        Address::<Frame>::zero(),
                        false,
                        PageAttributes::WRITABLE
                            | PageAttributes::NO_EXECUTE
                            | PageAttributes::DEMAND
                            | PageAttributes::USER
                            | PageAttributes::HUGE,
                    )
                    .unwrap();
            }

            TASK_STACK_BASE_ADDRESS.forward_checked(TASK_STACK_PAGE_COUNT).unwrap()
        };

        let mut global_tasks = scheduling::GLOBAL_TASKS.lock();
        global_tasks.push_back(scheduling::Task::new(
            scheduling::TaskPriority::new(scheduling::TaskPriority::MAX).unwrap(),
            // TODO account for memory base when passing entry offset
            scheduling::TaskStart::Address(Address::<Virtual>::new(driver.get_entry_offset() as u64).unwrap()),
            scheduling::TaskStack::At(stack_address.address()),
            {
                #[cfg(target_arch = "x86_64")]
                {
                    (
                        libarch::x64::cpu::GeneralContext::empty(),
                        libarch::x64::cpu::SpecialContext::flags_with_user_segments(
                            libarch::x64::registers::RFlags::INTERRUPT_FLAG,
                        ),
                    )
                }
            },
            #[cfg(target_arch = "x86_64")]
            {
                // TODO do not error here ?
                driver_page_manager.read_vmem_register().unwrap()
            },
        ))
    }
}
