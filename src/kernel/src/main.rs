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
    lang_items
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
extern crate libkernel;

mod arch;
mod boot;
mod drivers;
mod elf;
mod interrupts;
mod local_state;
mod memory;
mod num;
mod scheduling;
mod stdout;
mod tables;
mod time;

use core::{cell::OnceCell, sync::atomic::Ordering};
use spin::Once;

#[lang = "eh_personality"]
extern "C" fn rust_eh_personality() {}
#[no_mangle]
pub extern "C" fn _Unwind_Resume() {}

const MAXIMUM_STACK_TRACE_DEPTH: usize = 32;

fn trace_frame_pointer(
    stack_trace_addresses: &mut spin::MutexGuard<'static, [u64; MAXIMUM_STACK_TRACE_DEPTH]>,
) -> bool {
    #[repr(C, packed)]
    struct StackFrame {
        next_frame_ptr: *const StackFrame,
        return_address: u64,
    }

    let mut stack_trace_index: u8 = 0;
    let mut frame_ptr: *const StackFrame;
    // SAFETY: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };

    // SAFETY: Stack frame pointer should be valid, if `rbp` is being used correctly.
    // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
    while let Some(stack_frame) = unsafe { frame_ptr.as_ref() } {
        // 'Push' the return address to the
        stack_trace_addresses[stack_trace_index as usize] = stack_frame.return_address;
        frame_ptr = stack_frame.next_frame_ptr;

        // Increment the stack trace array index for the next iteration.
        match stack_trace_index.checked_add(1) {
            Some(value) => stack_trace_index = value,
            None => return true,
        }
    }

    false
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    static STACK_TRACE_ADDRESSES: spin::Mutex<[u64; MAXIMUM_STACK_TRACE_DEPTH]> =
        spin::Mutex::new([0u64; MAXIMUM_STACK_TRACE_DEPTH]);

    error!("KERNEL PANIC (at {}): {}", info.location().unwrap(), info.message().unwrap());

    let stack_traces = {
        let mut stack_trace_addresses = STACK_TRACE_ADDRESSES.lock();
        trace_frame_pointer(&mut stack_trace_addresses);
        stack_trace_addresses.clone()
    };
    crate::newline!();
    crate::println!("STACK TRACE:");

    let debug_tables = DEBUG_TABLES.get();

    for (increment, fn_address) in stack_traces.iter().rev().filter(|fn_address| **fn_address > 0).enumerate() {
        if let Some((symtab, strtab)) = debug_tables
            && let Some(fn_symbol) = symtab
                .iter()
                .filter(|symbol| symbol.get_type() == crate::elf::symbol::Type::Function)
                .find(|symbol| symbol.get_value() == *fn_address)
            && let Some(fn_name_offset) = fn_symbol.get_name_offset()
            && let Some(symbol_name) = core::ffi::CStr::from_bytes_until_nul(&strtab[fn_name_offset..])
                .ok()
                .and_then(|cstr| cstr.to_str().ok())
        {
            let tab = symbol_name.len() + increment;
            crate::println!("{symbol_name:tab$}");
        } else {
            let tab = (((u64::BITS - fn_address.leading_zeros()) / 4) as usize) + 2 + increment;
            crate::println!("{fn_address:#tab$X}");
        }
    }

    crate::interrupts::wait_loop()
}

#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    // TODO should we actually abort on every alloc error?
    crate::interrupts::wait_loop()
}

pub const LIMINE_REV: u64 = 0;
static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);
static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV).flags(0b1);
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);

#[used]
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

static CON_OUT: core::cell::SyncUnsafeCell<crate::memory::io::Serial> =
    core::cell::SyncUnsafeCell::new(crate::memory::io::Serial::new(crate::memory::io::COM1));
static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static SMP_MEMORY_INIT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

pub static DEBUG_TABLES: Once<(&[crate::elf::symbol::Symbol], &[u8])> = Once::new();

// TODO parse kernel command line configuration more succintly
static mut KERNEL_CFG_SMP: bool = false;

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[no_mangle]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    /* standard output setup */
    {
        let con_out_mut = &mut *CON_OUT.get();
        con_out_mut.init(crate::memory::io::SerialSpeed::S115200);
        crate::stdout::set_stdout(con_out_mut, log::LevelFilter::Trace);
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
        if let Some(vendor_str) = crate::arch::x64::cpu::get_vendor() {
            info!("Vendor              {vendor_str}");
        } else {
            info!("Vendor              None");
        }
    }

    let symbol_table = OnceCell::new();
    let string_table = OnceCell::new();

    /* parse kernel arguments */
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
                            if let Err(_) = symbol_table.set({
                                let data = section.data();
                                core::slice::from_raw_parts(data.as_ptr(), data.len())
                            }) {
                                break;
                            }
                        }

                        ".strtab" => {
                            if let Err(_) = string_table.set({
                                let data = section.data();
                                core::slice::from_raw_parts(data.as_ptr(), data.len())
                            }) {
                                break;
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
    }

    crate::memory::init_kernel_hhdm_address();
    crate::memory::init_kernel_frame_manager();
    crate::memory::init_kernel_page_manager();

    /* allocate the string & symbol tables */
    {
        let frame_manager = crate::memory::get_kernel_frame_manager();
        let hhdm_address = crate::memory::get_kernel_hhdm_address();

        if let Some(symbol_table) = symbol_table.get() && let Some(string_table) = string_table.get() {
            let symtab_frames_required = libkernel::align_up_div(symbol_table.len(), 0x1000);
            let strtab_frames_required = libkernel::align_up_div(string_table.len(), 0x1000);

            if let Ok(symtab_index) = frame_manager.lock_next_many(symtab_frames_required)
                && let Ok(strtab_index) = frame_manager.lock_next_many(strtab_frames_required)
            {
                let symtab = core::slice::from_raw_parts_mut(hhdm_address.as_mut_ptr::<u8>().add(symtab_index * 0x1000), symbol_table.len());
                let strtab = core::slice::from_raw_parts_mut(hhdm_address.as_mut_ptr::<u8>().add(strtab_index * 0x1000), string_table.len());

                symtab.copy_from_slice(symbol_table);
                strtab.copy_from_slice(string_table);

                DEBUG_TABLES.call_once(|| (bytemuck::cast_slice(symtab), strtab));
            }
        }
    }

    /* bsp core init */
    {
        #[cfg(target_arch = "x86_64")]
        {
            crate::arch::x64::cpu::load_registers();
            crate::arch::x64::cpu::load_tables();
        }

        // TODO rv64 bsp hart init
    }

    let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
    let page_manager = crate::memory::get_kernel_page_manager();
    let drivers_data = OnceCell::new();

    /* memory init */
    {
        use crate::memory::PageAttributes;
        use libkernel::{memory::Page, LinkerSymbol};

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

        let hhdm_base_page_index = crate::memory::get_kernel_hhdm_address().page_index();
        let hhdm_mapped_page = Page::from_index(hhdm_base_page_index);
        let old_page_manager = crate::memory::PageManager::from_current(&hhdm_mapped_page);

        // map code
        (__text_start.as_usize()..__text_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RX | PageAttributes::GLOBAL,
                        kernel_frame_manager,
                    )
                    .unwrap();
            });

        // map readonly
        (__rodata_start.as_usize()..__rodata_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RO | PageAttributes::GLOBAL,
                        kernel_frame_manager,
                    )
                    .unwrap();
            });

        // map readwrite
        (__bss_start.as_usize()..__bss_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RW | PageAttributes::GLOBAL,
                        kernel_frame_manager,
                    )
                    .unwrap();
            });

        (__data_start.as_usize()..__data_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RW | PageAttributes::GLOBAL,
                        kernel_frame_manager,
                    )
                    .unwrap();
            });

        kernel_frame_manager.iter().enumerate().for_each(|(frame_index, (_, ty))| {
            let page_attributes = {
                use crate::memory::FrameType;

                match ty {
                    FrameType::Unusable => PageAttributes::empty(),
                    FrameType::MMIO => PageAttributes::MMIO,
                    FrameType::Usable
                    | FrameType::Reserved
                    | FrameType::Kernel
                    | FrameType::FrameMap
                    | FrameType::BootReclaim
                    | FrameType::AcpiReclaim => PageAttributes::RW,
                }
            };

            page_manager
                .map(
                    &Page::from_index(hhdm_base_page_index + frame_index),
                    frame_index,
                    false,
                    page_attributes,
                    kernel_frame_manager,
                )
                .unwrap();
        });

        /* save drivers data */
        {
            if let Some(modules) =
                LIMINE_MODULES.get_response().get().map(|modules_response| modules_response.modules())
            {
                // Search specifically for 'drivers' module. This is a for loop to ensure forward compatibility if we ever load more than 1 module.

                for module in
                    modules.iter().filter(|module| module.path.to_str().unwrap().to_str().unwrap().ends_with("drivers"))
                {
                    let drivers_hhdm_ptr = module.base.as_ptr().unwrap();
                    for base_offset in (0..(module.length as usize)).step_by(0x1000) {
                        page_manager.set_page_attributes(
                            &libkernel::memory::Page::from_ptr(drivers_hhdm_ptr.add(base_offset)).unwrap(),
                            crate::memory::PageAttributes::RO,
                            crate::memory::AttributeModify::Set,
                        );
                    }

                    debug!("Read {} bytes of compressed drivers from memory.", module.length);
                    // Write pointer to driver data, so it is easily accessible after memory is switched to the kernel.
                    drivers_data.set(core::slice::from_raw_parts(drivers_hhdm_ptr, module.length as usize)).unwrap();
                }
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
        debug!("Detected {} additional cores.", smp_response.cpu_count);
        let bsp_lapic_id = smp_response.bsp_lapic_id;

        for cpu_info in smp_response.cpus() {
            if cpu_info.lapic_id != bsp_lapic_id {
                if KERNEL_CFG_SMP {
                    debug!("Starting processor: PID{}/LID{}", cpu_info.processor_id, cpu_info.lapic_id);

                    SMP_MEMORY_INIT.fetch_add(1, Ordering::Relaxed);
                    cpu_info.goto_address = _smp_entry as usize as u64;
                } else {
                    cpu_info.goto_address = crate::interrupts::wait_loop as usize as u64;
                }
            }
        }
    }

    /* memory finalize */
    {
        debug!("Switching to kernel page tables...");
        page_manager.write_cr3();
        debug!("Kernel has finalized control of page tables.");
        debug!("Assigning global allocator...");
        crate::memory::init_global_allocator(libkernel::memory::Page::from_index(
            ((384 * libkernel::memory::PML4_ENTRY_MEM_SIZE) / 0x1000) as usize,
        ));

        #[cfg(target_arch = "x86_64")]
        {
            debug!("Initializing APIC interface...");
            crate::arch::x64::structures::apic::init_interface(kernel_frame_manager, page_manager);
        }

        debug!("Initializing ACPI interface...");
        crate::tables::acpi::init_interface();
        debug!("Initializing PCI devices...");
        crate::memory::io::pci::init_devices();

        debug!("Boot core will release other cores, and then wait for all cores to update root page table.");
        SMP_MEMORY_READY.store(true, Ordering::Relaxed);

        while SMP_MEMORY_INIT.load(Ordering::Relaxed) > 0 {
            core::hint::spin_loop();
        }

        debug!("Reclaiming bootloader reclaimable memory...");
        crate::memory::reclaim_bootloader_frames();
    }

    /* configure I/O APIC redirections */
    #[cfg(target_arch = "x86_64")]
    {
        debug!("Configuring I/O APIC and processing interrupt overrides.");

        let ioapics = crate::arch::x64::structures::ioapic::get_io_apics();
        let platform_info = crate::tables::acpi::get_platform_info();

        if let acpi::platform::interrupt::InterruptModel::Apic(apic) = &platform_info.interrupt_model {
            let mut cur_vector = 0x70;

            for irq_source in apic.interrupt_source_overrides.iter() {
                debug!("{:?}", irq_source);

                let target_ioapic = ioapics
                    .iter()
                    .find(|ioapic| ioapic.handled_irqs().contains(&irq_source.global_system_interrupt))
                    .expect("no I/I APIC found for IRQ override");

                let mut redirection = target_ioapic.get_redirection(irq_source.global_system_interrupt);
                redirection.set_delivery_mode(interrupts::DeliveryMode::Fixed);
                redirection.set_destination_mode(interrupts::DestinationMode::Logical);
                redirection.set_masked(false);
                redirection.set_pin_polarity(irq_source.polarity);
                redirection.set_trigger_mode(irq_source.trigger_mode);
                redirection.set_vector({
                    let vector = cur_vector;
                    cur_vector += 1;
                    vector
                });
                redirection.set_destination_id(0 /* TODO real cpu id */);

                debug!(
                    "IRQ override: Global {} -> {}:{}",
                    irq_source.global_system_interrupt,
                    redirection.get_destination_id(),
                    redirection.get_vector()
                );
                target_ioapic.set_redirection(irq_source.global_system_interrupt, &redirection);
            }

            for nmi_source in apic.nmi_sources.iter() {
                debug!("{:?}", nmi_source);

                let target_ioapic = ioapics
                    .iter()
                    .find(|ioapic| ioapic.handled_irqs().contains(&nmi_source.global_system_interrupt))
                    .expect("no I/I APIC found for IRQ override");

                let mut redirection = target_ioapic.get_redirection(nmi_source.global_system_interrupt);
                redirection.set_delivery_mode(interrupts::DeliveryMode::NMI);
                redirection.set_destination_mode(interrupts::DestinationMode::Logical);
                redirection.set_masked(false);
                redirection.set_pin_polarity(nmi_source.polarity);
                redirection.set_trigger_mode(nmi_source.trigger_mode);
                redirection.set_vector({
                    let vector = cur_vector;
                    cur_vector += 1;
                    vector
                });
                redirection.set_destination_id(0 /* TODO real cpu id */);

                debug!(
                    "NMI override: Global {} -> {}:{}",
                    nmi_source.global_system_interrupt,
                    redirection.get_destination_id(),
                    redirection.get_vector()
                );
                target_ioapic.set_redirection(nmi_source.global_system_interrupt, &redirection);
            }
        }

        /* enable ACPI SCI interrupts */
        {
            // TODO clean this filthy mess up

            let pm1a_evt_blk =
                &crate::tables::acpi::get_fadt().pm1a_event_block().expect("no `PM1a_EVT_BLK` found in FADT");

            let mut reg = crate::tables::acpi::Register::<u16>::IO(crate::memory::io::ReadWritePort::new(
                (pm1a_evt_blk.address + ((pm1a_evt_blk.bit_width / 8) as u64)) as u16,
            ));

            reg.write((1 << 8) | (1 << 0));
        }
    }

    info!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, Ordering::Relaxed);

    {
        use crate::{elf::segment, memory::PageAttributes};
        use libkernel::memory::Page;

        let drivers_raw_data = miniz_oxide::inflate::decompress_to_vec(drivers_data.get().unwrap()).unwrap();
        debug!("Decompressed {} bytes of driver files.", drivers_raw_data.len());

        let frame_manager = crate::memory::get_kernel_frame_manager();
        let hhdm_address = crate::memory::get_kernel_hhdm_address();

        // Iterate and load drivers as tasks.
        let mut current_offset = 0;
        loop {
            // Copy and reconstruct the driver byte length from the prefix.
            let driver_len = {
                let mut value = 0;

                value |= (drivers_raw_data[current_offset + 0] as u64) << 0;
                value |= (drivers_raw_data[current_offset + 1] as u64) << 8;
                value |= (drivers_raw_data[current_offset + 2] as u64) << 16;
                value |= (drivers_raw_data[current_offset + 3] as u64) << 24;
                value |= (drivers_raw_data[current_offset + 4] as u64) << 32;
                value |= (drivers_raw_data[current_offset + 5] as u64) << 40;
                value |= (drivers_raw_data[current_offset + 6] as u64) << 48;
                value |= (drivers_raw_data[current_offset + 7] as u64) << 56;

                value as usize
            };

            let base_offset = current_offset + 8 /* skip 'len' prefix */;
            let driver_data = &drivers_raw_data[base_offset..(base_offset + driver_len)];
            let driver_elf = crate::elf::Elf::from_bytes(driver_data).unwrap();
            info!("{:?}", driver_elf);

            // Create the driver's page manager from the kernel's higher-half table.
            let driver_page_manager = crate::memory::PageManager::new(
                frame_manager,
                &Page::from_address(crate::memory::get_kernel_hhdm_address()).unwrap(),
                Some(crate::memory::get_kernel_page_manager().copy_pml4()),
            );

            // Iterate the segments, and allocate them.
            for segment in driver_elf.iter_segments() {
                trace!("{:?}", segment);

                match segment.get_type() {
                    segment::Type::Loadable => {
                        let memory_start = segment.get_virtual_address().unwrap().as_usize();
                        let memory_end = memory_start + segment.get_memory_layout().unwrap().size();
                        let start_page_index = libkernel::align_down_div(memory_start, 0x1000);
                        let end_page_index = libkernel::align_up_div(memory_end, 0x1000);
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

                            let page = Page::from_index(page_index);
                            let frame_index = frame_manager.lock_next().unwrap();
                            driver_page_manager
                                .map(&page, frame_index, false, page_attributes | PageAttributes::USER, frame_manager)
                                .unwrap();

                            // SAFETY: HHDM is guaranteed by kernel to be valid, and the frame being pointed to was just allocated.
                            let memory_hhdm = unsafe {
                                core::slice::from_raw_parts_mut(
                                    (hhdm_address.as_usize() + (frame_index * 0x1000)) as *mut u8,
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
                use libkernel::{Address, Physical, Virtual};

                let stack_address = {
                    // TODO make this a dynamic configuration
                    const DEFAULT_TASK_STACK_SIZE: u64 = 4 * libkernel::MIBIBYTE;

                    let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
                    let base_address = Address::<Virtual>::new_truncate(128 * libkernel::memory::PML4_ENTRY_MEM_SIZE);
                    let stack_address =
                        Address::<Virtual>::new_truncate(base_address.as_u64() + DEFAULT_TASK_STACK_SIZE);
                    for page_index in base_address.page_index()..stack_address.page_index() {
                        driver_page_manager
                            .map(
                                &Page::from_index(page_index),
                                0,
                                false,
                                PageAttributes::DEMAND | PageAttributes::USER,
                                kernel_frame_manager,
                            )
                            .unwrap();
                    }

                    stack_address
                };

                let mut global_tasks = scheduling::GLOBAL_TASKS.lock();
                global_tasks.push_back(scheduling::Task::new(
                    scheduling::TaskPriority::new(scheduling::TaskPriority::MAX).unwrap(),
                    // TODO account for memory base when passing entry offset
                    scheduling::TaskStart::Address(
                        Address::<Virtual>::new(driver_elf.get_entry_offset() as u64).unwrap(),
                    ),
                    scheduling::TaskStack::At(stack_address),
                    {
                        #[cfg(target_arch = "x86_64")]
                        {
                            (
                                crate::arch::x64::cpu::GeneralContext::empty(),
                                crate::arch::x64::cpu::SpecialContext::with_user_segments(
                                    crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
                                ),
                            )
                        }
                    },
                    #[cfg(target_arch = "x86_64")]
                    {
                        crate::memory::RootPageTable(
                            Address::<Physical>::new((driver_page_manager.root_frame_index() * 0x1000) as u64).unwrap(),
                            crate::arch::x64::registers::control::CR3Flags::empty(),
                        )
                    },
                ))
            }

            current_offset += driver_len + 8  /* skip 'len' prefix */;
            if current_offset >= drivers_raw_data.len() {
                break;
            }
        }
    }

    kernel_thread_setup()
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
unsafe fn _smp_entry() -> ! {
    // Wait to ensure the machine is the correct state to execute cpu setup.
    while !SMP_MEMORY_READY.load(Ordering::Relaxed) {
        core::hint::spin_loop();
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::cpu::load_registers();
        crate::arch::x64::cpu::load_tables();
    }

    crate::memory::get_kernel_page_manager().write_cr3();

    SMP_MEMORY_INIT.fetch_sub(1, Ordering::Relaxed);
    while SMP_MEMORY_INIT.load(Ordering::Relaxed) > 0 {
        core::hint::spin_loop();
    }

    trace!("Finished SMP entry for core.");

    kernel_thread_setup()
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup() -> ! {
    crate::local_state::init(0);

    trace!("Beginning scheduling...");
    crate::local_state::try_begin_scheduling();
    crate::interrupts::wait_loop()
}
