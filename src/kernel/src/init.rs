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
mod logging;
mod panic;
mod proc;
mod rand;
mod time;

use libkernel::LinkerSymbol;
use libsys::Address;

pub static KERNEL_HANDLE: spin::Lazy<uuid::Uuid> = spin::Lazy::new(|| uuid::Uuid::new_v4());

/// ### Safety
///
/// This function should only ever be called by the bootloader.
#[no_mangle]
#[doc(hidden)]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    // Logging isn't set up, so we'll just spin loop if we fail to initialize it.
    crate::logging::init().unwrap_or_else(|_| crate::interrupts::wait_loop());

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

    crate::memory::with_kmapper(|kmapper| {
        use crate::memory::{address_space::Mapper, hhdm_address, PageAttributes, PageDepth};

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
        unsafe { kmapper.swap_into() };
        debug!("Kernel has finalized control of page tables.");
    });

    debug!("Initializing ACPI interface...");
    crate::acpi::init_interface();

    /* symbols */
    if !crate::boot::PARAMETERS.low_memory {
        debug!("Parsing kernel symbols...");

        let (kernel_file_base, kernel_file_len) = {
            let kernel_file = crate::boot::get_kernel_file().expect("failed to get kernel file");
            (kernel_file.base.as_ptr().unwrap(), kernel_file.length as usize)
        };

        let kernel_elf = libkernel::elf::Elf::from_bytes(
            // Safety: Kernel file is guaranteed to be valid by bootloader.
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

                cpu_info.goto_address = if crate::boot::PARAMETERS.smp {
                    extern "C" fn _smp_entry(info: *const limine::LimineSmpInfo) -> ! {
                        crate::cpu::setup();

                        // Safety: All currently referenced memory should also be mapped in the kernel page tables.
                        crate::memory::with_kmapper(|kmapper| unsafe { kmapper.swap_into() });

                        // Safety: Function is called only once for this core.
                        unsafe { kernel_core_setup(info.read().lapic_id) }
                    }

                    _smp_entry
                } else {
                    extern "C" fn _idle_forever(_: *const limine::LimineSmpInfo) -> ! {
                        // Safety: Murder isn't legal. Is this?
                        unsafe { crate::interrupts::halt_and_catch_fire() }
                    }

                    _idle_forever
                };
            }
        } else {
            debug!("Bootloader has not provided any SMP information.");
        }
    }

    debug!("Reclaiming bootloader memory...");
    crate::boot::reclaim_boot_memory();
    debug!("Bootloader memory reclaimed.");

    kernel_core_setup(0)
}

/// ### Safety
///
/// This function should only ever be called once per core.
#[inline(never)]
pub(self) unsafe fn kernel_core_setup(core_id: u32) -> ! {
    crate::local_state::init(core_id, 1000);

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();
    crate::local_state::begin_scheduling();

    // This interrupt wait loop is necessary to ensure the core can jump into the scheduler.
    crate::interrupts::wait_loop()
}

pub fn load_drivers() {
    let drivers_data = crate::boot::get_kernel_modules()
        // Find the drives module, and map the `Option<>` to it.
        .and_then(|modules| {
            modules.iter().find(|module| module.path.to_str().unwrap().to_str().unwrap().ends_with("drivers"))
        })
        // Safety: Kernel promises HHDM to be valid, and the module pointer should be in the HHDM, so this should be valid for `u8`.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");

    let archive = tar_no_std::TarArchiveRef::new(drivers_data);

    for archive_entry in archive.entries() {
        use crate::memory::{PageAttributes, PageDepth};
        use libkernel::elf::segment;
        use libsys::{page_shift, page_size};

        debug!("Processing archive entry for driver: {}", archive_entry.filename());

        let driver_elf =
            libkernel::elf::Elf::from_bytes(archive_entry.data()).expect("failed to parse driver blob into valid ELF");

        trace!("{:?}", driver_elf);

        // Create the driver's page manager from the kernel's higher-half table.
        // Safety: Kernel guarantees HHDM to be valid.
        let mut driver_mapper = unsafe {
            crate::memory::address_space::Mapper::new_unsafe(
                PageDepth::new(4),
                crate::memory::new_kmapped_page_table().unwrap(),
            )
        };

        // Iterate the segments, and allocate them.
        for segment in driver_elf.iter_segments() {
            trace!("{:?}", segment);

            match segment.get_type() {
                segment::Type::Loadable => {
                    let memory_size = segment.get_memory_layout().unwrap().size();
                    let memory_start = segment.get_virtual_address().unwrap().get();
                    let memory_end = memory_start + memory_size;

                    // Align the start address to ensure we iterate page-aligned addresses.
                    let memory_start_aligned = libsys::align_down(memory_start, page_shift());
                    for page_base in (memory_start_aligned..memory_end).step_by(page_size().get()) {
                        let page = Address::new(page_base).unwrap();
                        // Auto map the virtual address to a physical page.
                        driver_mapper
                            .auto_map(page, {
                                // This doesn't support RWX pages. I'm not sure it ever should.
                                if segment.get_flags().contains(segment::Flags::EXECUTABLE) {
                                    PageAttributes::RX
                                } else if segment.get_flags().contains(segment::Flags::WRITABLE) {
                                    PageAttributes::RW
                                } else {
                                    PageAttributes::RO
                                }
                            })
                            .unwrap();
                    }

                    let segment_slice = segment.data();
                    // Safety: `memory_start` pointer is valid as we just mapped all of the requisite pages for `memory_size` length.
                    let memory_slice = unsafe { core::slice::from_raw_parts(memory_start as *mut u8, memory_size) };
                    // Copy segment data into the new memory region.
                    memory_slice[..segment_slice.len()].copy_from_slice(segment_slice);
                    // Clear any left over bytes to 0. This is useful for the bss region, for example.
                    (&memory_slice[segment_slice.len()..]).fill(0x0);
                }

                _ => {}
            }
        }
    }
}

/* load driver */

// Push ELF as global task.

// let stack_address = {
//     const TASK_STACK_BASE_ADDRESS: Address<Page> = Address::<Page>::new_truncate(
//         Address::<Virtual>::new_truncate(128 << 39),
//         Some(PageAlign::Align2MiB),
//     );
//     // TODO make this a dynamic configuration
//     const TASK_STACK_PAGE_COUNT: usize = 2;

//     for page in (0..TASK_STACK_PAGE_COUNT)
//         .map(|offset| TASK_STACK_BASE_ADDRESS.forward_checked(offset).unwrap())
//     {
//         driver_page_manager
//             .map(
//                 page,
//                 Address::<Frame>::zero(),
//                 false,
//                 PageAttributes::WRITABLE
//                     | PageAttributes::NO_EXECUTE
//                     | PageAttributes::DEMAND
//                     | PageAttributes::USER
//                     | PageAttributes::HUGE,
//             )
//             .unwrap();
//     }

//     TASK_STACK_BASE_ADDRESS.forward_checked(TASK_STACK_PAGE_COUNT).unwrap()
// };

// TODO
// let task = crate::local_state::Task::new(
//     u8::MIN,
//     // TODO account for memory base when passing entry offset
//     crate::local_state::EntryPoint::Address(
//         Address::<Virtual>::new(elf.get_entry_offset() as u64).unwrap(),
//     ),
//     stack_address.address(),
//     {
//         #[cfg(target_arch = "x86_64")]
//         {
//             (
//                 crate::arch::x64::registers::GeneralRegisters::empty(),
//                 crate::arch::x64::registers::SpecialRegisters::flags_with_user_segments(
//                     crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
//                 ),
//             )
//         }
//     },
//     #[cfg(target_arch = "x86_64")]
//     {
//         // TODO do not error here ?
//         driver_page_manager.read_vmem_register().unwrap()
//     },
// );

// crate::local_state::queue_task(task);
