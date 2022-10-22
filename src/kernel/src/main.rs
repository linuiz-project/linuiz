#![no_std]
#![no_main]
#![feature(
    asm_const,
    naked_functions,
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
    exact_size_is_empty,
    fn_align
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
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

#[macro_use]
extern crate log;
extern crate libcommon;
extern crate lzalloc;

mod acpi;
mod arch;
mod boot;
mod cpu;
mod elf;
mod interrupts;
mod local_state;
mod memory;
mod modules;
mod num;
mod panic;
mod time;

use libcommon::{Address, Frame, Page, Virtual};

pub type MmapEntry = limine::NonNullPtr<limine::LimineMemmapEntry>;
pub type MmapEntryType = limine::LimineMemoryMapEntryType;

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub smp: bool,
    pub symbolinfo: bool,
    pub low_memory: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { smp: true, symbolinfo: false, low_memory: false }
    }
}

static PARAMETERS: spin::Lazy<Parameters> = spin::Lazy::new(|| {
    crate::boot::get_kernel_file()
        .and_then(|kernel_file| kernel_file.cmdline.to_str())
        .and_then(|cmdline_cstr| cmdline_cstr.to_str().ok())
        .map(|cmdline| {
            let mut parameters = Parameters::default();

            for parameter in cmdline.split(' ') {
                match parameter.split_once(':') {
                    Some(("smp", "on")) => parameters.smp = true,
                    Some(("smp", "off")) => parameters.smp = false,

                    None if parameter == "symbolinfo" => parameters.symbolinfo = true,
                    None if parameter == "lomem" => parameters.low_memory = true,

                    _ => warn!("Unhandled cmdline parameter: {:?}", parameter),
                }
            }

            parameters
        })
        .unwrap_or_default()
});

// TODO somehow model that these requests are invalidated

/// SAFETY: Do not call this function.
#[no_mangle]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    log::set_max_level(log::LevelFilter::Trace);
    log::set_logger({
        static UART: spin::Lazy<crate::memory::io::Serial> = spin::Lazy::new(|| {
            // SAFETY: Function is called only once, when the `Lazy` is initialized.
            unsafe { crate::memory::io::Serial::init() }
        });

        &*UART
    })
    .unwrap();

    /* misc. boot info */
    {
        static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(crate::boot::LIMINE_REV);

        if let Some(boot_info) = LIMINE_INFO.get_response().get() {
            use core::ffi::CStr;

            info!(
                "Bootloader Info     {} v{} (rev {})",
                boot_info
                    .name
                    .as_ptr()
                    .map(|ptr| unsafe { CStr::from_ptr(ptr) })
                    .and_then(|cstr| cstr.to_str().ok())
                    .unwrap_or("Unknown"),
                boot_info
                    .version
                    .as_ptr()
                    .map(|ptr| unsafe { CStr::from_ptr(ptr) })
                    .and_then(|cstr| cstr.to_str().ok())
                    .unwrap_or("0"),
                boot_info.revision
            );
        }

        // Vendor strings from the CPU need to be enumerated per-platform.
        #[cfg(target_arch = "x86_64")]
        if let Some(vendor_info) = crate::arch::x64::cpuid::VENDOR_INFO.as_ref() {
            info!("Vendor              {}", vendor_info.as_str());
        } else {
            info!("Vendor              Unknown");
        }
    }

    crate::cpu::setup();

    /*
     * Memory
     */
    {
        use crate::memory::{Mapper, PageAttributes};
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

            static __section_align: LinkerSymbol;
        }

        // TODO constant value for minimum page size
        //let boot_page_table = PageTable::new(4, hhdm_address, entry)

        fn map_range_from(
            from_mapper: &Mapper,
            to_mapper: &Mapper,
            range: core::ops::Range<u64>,
            attributes: PageAttributes,
        ) {
            // SAFETY: Linker should have correctly set this value.
            let page_align = unsafe { PageAlign::from_u64(__section_align.as_u64()).unwrap() };

            for address in range.step_by(page_align.as_usize()).map(Address::<Virtual>::new_truncate) {
                to_mapper
                    .map(
                        Address::<Page>::new_truncate(address, page_align),
                        // Properly handle the bootloader possibly not using huge page mappings (but still physically contiguous).
                        Address::<Page>::new(address, None).and_then(|page| from_mapper.get_mapped_to(page)).unwrap(),
                        false,
                        attributes,
                    )
                    .unwrap();
            }
        }

        // TODO don't unwrap, possibly switch to a simpler allocator? bump allocator, maybe
        // SAFETY: Kernel guarantees the HHDM to be valid.

        debug!("Initializing kernel mapper...");
        // SAFETY: Kernel guarantees HHDM address to be valid.
        let boot_mapper = unsafe { Mapper::from_current(memory::get_hhdm_address()) };
        let kernel_mapper = memory::get_kernel_mapper();

        /* map the kernel segments */
        {
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __text_start.as_u64()..__text_end.as_u64() },
                PageAttributes::RX | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __rodata_start.as_u64()..__rodata_end.as_u64() },
                PageAttributes::RO | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __bss_start.as_u64()..__bss_end.as_u64() },
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
            map_range_from(
                &boot_mapper,
                &kernel_mapper,
                // SAFETY: These linker symbols are guaranteed by the bootloader to be valid.
                unsafe { __data_start.as_u64()..__data_end.as_u64() },
                PageAttributes::RW | PageAttributes::GLOBAL,
            );
        }

        for entry in crate::boot::get_memory_map().unwrap() {
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
                kernel_mapper
                    .map(
                        Address::<libcommon::Page>::new(
                            Address::<Virtual>::new(crate::memory::get_hhdm_address().as_u64() + phys_base).unwrap(),
                            Some(PageAlign::Align4KiB),
                        )
                        .unwrap(),
                        Address::<Frame>::new(phys_base).unwrap(),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }
        }

        debug!("Switching to kernel page tables...");
        // SAFETY: Kernel mapper has mapped all existing memory references, so commiting
        //         changes nothing from the software perspective.
        unsafe { kernel_mapper.commit_vmem_register() }.unwrap();
        debug!("Kernel has finalized control of page tables.");
    }

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* symbols */
    if !PARAMETERS.low_memory {
        let (kernel_file_base, kernel_file_len) = {
            let kernel_file = crate::boot::get_kernel_file().unwrap();
            (kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize)
        };

        let kernel_elf = crate::elf::Elf::from_bytes(
            // SAFETY: Kernel file is guaranteed to be valid by bootloader.
            unsafe { core::slice::from_raw_parts(kernel_file_base, kernel_file_len) },
        )
        .expect("failed to parse kernel executable");
        if let Some(names_section) = kernel_elf.get_section_names_section() {
            for section in kernel_elf.iter_sections() {
                use lzalloc::vec::Vec;

                let names_section_offset = section.get_names_section_offset();
                if names_section.data().len() > names_section_offset {
                    continue;
                }

                let Some(section_name) = core::ffi::CStr::from_bytes_until_nul(&names_section.data()[names_section_offset..])
                        .ok()
                        .and_then(|cstr| cstr.to_str().ok())
                    else { continue };

                if section_name == ".symtab" && let Ok(symbols) = bytemuck::try_cast_slice(section.data()) {
                    crate::panic::KERNEL_SYMBOLS.call_once(|| {
                        let mut symbols_copy = Vec::new();
                        symbols_copy.extend_from_slice(symbols);
                        symbols_copy
                    });
                } else if section_name == ".strtab" {
                    crate::panic::KERNEL_STRINGS.call_once(|| {
                        let mut strings_copy = Vec::new();
                        strings_copy.extend_from_slice(section.data());
                        strings_copy
                    });
                }
            }
        }
    } else {
        debug!("Kernel is running in low memory mode; stack tracing will be disabled.");
    }

    /* smp */
    {
        static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(crate::boot::LIMINE_REV)
            // Enable x2APIC mode if available.
            .flags(0b1);

        if let Some(smp_response) = LIMINE_SMP.get_response().get_mut() {
            let bsp_lapic_id = smp_response.bsp_lapic_id;
            debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
            for cpu_info in smp_response.cpus().iter_mut().filter(|info| info.lapic_id != bsp_lapic_id) {
                trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

                cpu_info.goto_address = if PARAMETERS.smp {
                    extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                        crate::cpu::setup();

                        // SAFETY: All currently referenced memory should also be mapped in the kernel page tables.
                        unsafe { crate::memory::get_kernel_mapper().commit_vmem_register().unwrap() };

                        // SAFETY: Function is called only once for this core.
                        unsafe { crate::kernel_thread_setup(info.read().lapic_id) }
                    }

                    _smp_entry
                } else {
                    extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                        // SAFETY: Murder isn't legal. Is this?
                        unsafe { crate::interrupts::halt_and_catch_fire() }
                    }

                    _idle_forever
                };
            }
        } else {
            debug!("Bootloader has not provided any SMP information.");
        }
    }

    /* arch core init */
    // #[cfg(target_arch = "x86_64")]
    // {
    //     // SAFETY: Provided IRQ base is intentionally within the exception range for x86 CPUs.
    //     static PICS: spin::Mutex<pic_8259::Pics> = spin::Mutex::new(unsafe { pic_8259::Pics::new(0) });
    //     PICS.lock().init(pic_8259::InterruptLines::empty());
    // }

    // TODO rv64 bsp hart init

    // Because the SMP information structures (and thus, their `goto_address`) are only mapped in the bootloader
    // page tables, we have to start the other cores and pass the root page table frame index in. All of the cores
    // will then wait until every core has swapped to the new page tables, then this core (the boot core) will
    // reclaim bootloader memory.

    /* memory finalize */
    {
        // TODO debug!("Loading pre-packaged drivers...");
        // load_drivers();

        // TODO init PCI devices
        // debug!("Initializing PCI devices...");
        // crate::memory::io::pci::init_devices();

        // TODO reclaim bootloader memory
        // debug!("Reclaiming bootloader reclaimable memory...");
        // crate::memory::reclaim_bootloader_frames();
    }

    /* configure I/O APIC redirections */
    #[cfg(target_arch = "x86_64")]
    {
        //     debug!("Configuring I/O APIC and processing interrupt overrides.");

        //     let ioapics = crate::arch::x64::structures::ioapic::get_io_apics();
        //     let platform_info = crate::acpi::get_platform_info();

        //     if let acpi::platform::interrupt::InterruptModel::Apic(apic) = &platform_info.interrupt_model {
        //         use crate::interrupts;

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
    }

    // TODO make this a standalone function so we can return error states

    kernel_thread_setup(0)
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup(core_id: u32) -> ! {
    crate::local_state::init(core_id, 1000);

    crate::interrupts::enable();
    crate::local_state::begin_scheduling();
    trace!("Core #{} scheduled.", core_id);
    crate::interrupts::wait_loop()
}
