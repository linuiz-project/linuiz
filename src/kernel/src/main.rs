#![no_std]
#![no_main]
#![feature(
    result_flattening,                      // #70142 <https://github.com/rust-lang/rust/issues/70142>
    map_try_insert,                         // #82766 <https://github.com/rust-lang/rust/issues/82766>
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
    if_let_guard,
    exact_size_is_empty,
    fn_align,
    ptr_as_uninit,
    nonnull_slice_from_raw_parts,
    ptr_metadata,
    control_flow_enum,
    btreemap_alloc,
    inline_const,
    const_option,
    const_option_ext,
    const_trait_impl,
    const_cmp
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

extern crate alloc;
#[macro_use]
extern crate log;

#[cfg(target_pointer_width = "32")]
#[allow(non_camel_case_types)]
pub type psize = u32;
#[cfg(target_pointer_width = "64")]
#[allow(non_camel_case_types)]
pub type psize = u64;

mod acpi;
mod arch;
mod boot;
mod cpu;
mod exceptions;
mod interrupts;
mod local_state;
mod memory;
// mod modules;
mod panic;
mod proc;
mod time;

use libkernel::LinkerSymbol;
use libsys::Address;

#[cfg(not(target_arch = "x86_64"))]
getrandom::register_custom_getrandom!({ todo!() });

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(|| uuid::Uuid::new_v4());

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

/// ### Safety
///
/// Do not call this function.
#[no_mangle]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    #[cfg(debug_assertions)]
    {
        log::set_max_level(log::LevelFilter::Trace);
    }
    #[cfg(not(debug_assertions))]
    {
        log::set_max_level(log::LevelFilter::Info);
    }

    log::set_logger({
        static UART: spin::Lazy<crate::memory::io::Serial> = spin::Lazy::new(|| {
            // ### Safety: Function is called only once, when the `Lazy` is initialized.
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

    memory::with_kmapper(|kmapper| {
        use memory::{address_space::Mapper, hhdm_address, PageAttributes, PageDepth};

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

        debug!("Initializing kernel mapper...");


        fn map_range_from(
            from_mapper: &Mapper,
            to_mapper: &mut Mapper,
            range: core::ops::Range<usize>,
            attributes: PageAttributes,
        ) {
            trace!("{:X?} [{:?}]", range, attributes);

            for address in range.step_by(0x1000) {
                to_mapper
                    .map(
                        Address::new_truncate(address),
                        PageDepth::min(),
                        from_mapper.get_mapped_to(Address::new_truncate(address)).unwrap(),
                        false,
                        attributes,
                    )
                    .unwrap();
            }
        }

        // Safety: All parameters are provided from valid sources.
        let boot_mapper =
            unsafe { Mapper::new_unsafe(PageDepth::current(), crate::memory::PagingRegister::read().frame()) };

        /* map the kernel segments */
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __text_start.as_ptr::<u8>().addr()..__text_end.as_ptr::<u8>().addr() },
            PageAttributes::RX | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __rodata_start.as_ptr::<u8>().addr()..__rodata_end.as_ptr::<u8>().addr() },
            PageAttributes::RO | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __bss_start.as_ptr::<u8>().addr()..__bss_end.as_ptr::<u8>().addr() },
            PageAttributes::RW | PageAttributes::GLOBAL,
        );
        map_range_from(
            &boot_mapper,
            kmapper,
            // Safety: These linker symbols are guaranteed by the bootloader to be valid.
            unsafe { __data_start.as_ptr::<u8>().addr()..__data_end.as_ptr::<u8>().addr() },
            PageAttributes::RW | PageAttributes::GLOBAL,
        );

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

            for phys_base in (entry.base..(entry.base + entry.len)).step_by(0x1000).map(|p| p as usize) {
                kmapper
                    .map(
                        Address::new_truncate(hhdm_address().get() + phys_base),
                        PageDepth::min(),
                        Address::new_truncate(phys_base as usize),
                        false,
                        page_attributes,
                    )
                    .unwrap();
            }

            // ... map architecture-specific memory ...

            #[cfg(target_arch = "x86_64")]
            {
                // map APIC ...
                let apic_address = msr::IA32_APIC_BASE::get_base_address() as usize;
                kmapper
                    .map(
                        Address::new_truncate(hhdm_address().get() + apic_address),
                        PageDepth::min(),
                        Address::new_truncate(apic_address),
                        false,
                        PageAttributes::MMIO,
                    )
                    .unwrap();
            }
        }

        debug!("Switching to kernel page tables...");
        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe { kmapper.commit_vmem_register() }.unwrap();
        debug!("Kernel has finalized control of page tables.");
    });

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* symbols */
    if !PARAMETERS.low_memory {
        debug!("Parsing kernel symbols...");

        let (kernel_file_base, kernel_file_len) = {
            let kernel_file = crate::boot::get_kernel_file().expect("failed to get kernel file");
            (kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize)
        };

        let kernel_elf = libkernel::elf::Elf::from_bytes(
            // ### Safety: Kernel file is guaranteed to be valid by bootloader.
            unsafe { core::slice::from_raw_parts(kernel_file_base, kernel_file_len) },
        )
        .expect("failed to parse kernel executable");

        if let Some(names_section) = kernel_elf.get_section_names_section() {
            let names_section = names_section.data();

            for section in kernel_elf.iter_sections() {
                use libkernel::elf::symbol::Symbol;
                use try_alloc::boxed::TryBox;

                let names_section_offset = section.get_names_section_offset();
                // Check if names section offset is greater than the length of the names section.
                if names_section.len() < names_section_offset {
                    continue;
                }

                let section_data = section.data();
                let Some(section_name) = core::ffi::CStr::from_bytes_until_nul(&names_section[names_section_offset..])
                        .ok()
                        .and_then(|cstr| cstr.to_str().ok())
                    else { continue };

                match section_name {
                    ".symtab" if section_data.len() > 0 => {
                        let symbols = {
                            let (pre, symbols, post) = section_data.align_to::<Symbol>();

                            debug_assert!(pre.is_empty());
                            debug_assert!(post.is_empty());

                            symbols
                        };

                        let Ok(mut symbols_copy) = TryBox::new_slice(symbols.len(), Symbol::default()) else { continue };

                        crate::interrupts::without(|| {
                            crate::panic::KERNEL_SYMBOLS.call_once(|| {
                                symbols_copy.copy_from_slice(symbols);
                                TryBox::leak(symbols_copy)
                            });
                        });
                    }

                    ".strtab" if section_data.len() > 0 => {
                        let Ok(mut strings_copy) = TryBox::new_slice(section_data.len(), 0) else { continue };

                        crate::interrupts::without(|| {
                            crate::panic::KERNEL_STRINGS.call_once(|| {
                                strings_copy.copy_from_slice(section.data());
                                TryBox::leak(strings_copy)
                            });
                        });
                    }

                    _ => {}
                }
            }
        }
    } else {
        debug!("Kernel is running in low memory mode; pretty stack tracing will be disabled.");
    }

    // TODO modules
    // debug!("Loading kernel modules...");
    // crate::modules::load_modules();

    /* smp */
    {
        static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(crate::boot::LIMINE_REV)
            // Enable x2APIC mode if available.
            .flags(0b1);

        debug!("Detecting and starting additional cores.");

        if let Some(smp_response) = LIMINE_SMP.get_response().get_mut() {
            let bsp_lapic_id = smp_response.bsp_lapic_id;
            debug!("Detected {} additional cores.", smp_response.cpu_count - 1);
            for cpu_info in smp_response.cpus().iter_mut().filter(|info| info.lapic_id != bsp_lapic_id) {
                trace!("Starting processor: ID P{}/L{}", cpu_info.processor_id, cpu_info.lapic_id);

                cpu_info.goto_address = if PARAMETERS.smp {
                    extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                        crate::cpu::setup();

                        // Safety: All currently referenced memory should also be mapped in the kernel page tables.
                        crate::memory::with_kmapper(|kmapper| unsafe { kmapper.commit_vmem_register().unwrap() });

                        // Safety: Function is called only once for this core.
                        unsafe { crate::kernel_thread_setup(info.read().lapic_id) }
                    }

                    _smp_entry
                } else {
                    extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                        // ### Safety: Murder isn't legal. Is this?
                        unsafe { crate::interrupts::halt_and_catch_fire() }
                    }

                    _idle_forever
                };
            }
        } else {
            debug!("Bootloader has not provided any SMP information.");
        }
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

        //     //     let pm1a_evt_blk =
        //     //         &crate::tables::acpi::get_fadt().pm1a_event_block().expect("no `PM1a_EVT_BLK` found in FADT");

        //     //     let mut reg = libsys::acpi::Register::<u16>::IO(crate::memory::io::ReadWritePort::new(
        //     //         (pm1a_evt_blk.address + ((pm1a_evt_blk.bit_width / 8) as u64)) as u16,
        //     //     ));

        //     //     reg.write((1 << 8) | (1 << 0));
        //     // }
    }

    debug!("Reclaiming bootloader memory...");
    crate::boot::reclaim_boot_memory();

    kernel_thread_setup(0)
}

/// ### Safety
///
/// This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup(core_id: u32) -> ! {
    crate::local_state::init(core_id, 1000);

    crate::interrupts::enable();
    crate::local_state::begin_scheduling();
    crate::interrupts::wait_loop()
}
