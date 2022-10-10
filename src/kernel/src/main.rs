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

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libcommon;

mod acpi;
mod elf;
mod init;
mod local_state;
mod memory;
mod modules;
mod num;
mod panic;
mod syscall;
mod time;

use libcommon::Address;

pub const LIMINE_REV: u64 = 0;
// TODO somehow model that these requests are invalidated
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);
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

pub type MmapEntry = limine::NonNullPtr<limine::LimineMemmapEntry>;
pub type MmapEntryType = limine::LimineMemoryMapEntryType;

/// SAFETY: Do not call this function in software.
#[no_mangle]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    /* arch core init */
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: Provided IRQ base is intentionally within the exception range for x86 CPUs.
        static PICS: spin::Mutex<pic_8259::Pics> = spin::Mutex::new(unsafe { pic_8259::Pics::new(0) });
        PICS.lock().init(pic_8259::InterruptLines::empty());
    }

    init::init();

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
    }

    // TODO make this a standalone function so we can return error states

    kernel_thread_setup(0)
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup(core_id: u32) -> ! {
    crate::local_state::init(core_id);

    libarch::interrupts::enable();
    trace!("Enabling and starting scheduler...");
    crate::local_state::with_scheduler(crate::local_state::Scheduler::start);
    trace!("Core will soon execute a task, or otherwise halt.");
    libarch::interrupts::wait_loop()
}

#[doc(hidden)]
mod handlers {
    use libcommon::{Address, Page, Virtual};

    /// SAFETY: Do not call this function.
    #[no_mangle]
    #[repr(align(0x10))]
    unsafe fn __pf_handler(address: Address<Virtual>) -> Result<(), libarch::interrupts::PageFaultHandlerError> {
        use crate::memory::PageAttributes;
        use libarch::interrupts::PageFaultHandlerError;

        let fault_page = Address::<Page>::new(address, None).unwrap();
        let virtual_mapper = crate::memory::Mapper::from_current(crate::memory::get_hhdm_address());
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

    /// SAFETY: Do not call this function.
    #[no_mangle]
    #[repr(align(0x10))]
    unsafe fn __irq_handler(
        irq_vector: u64,
        ctrl_flow_context: &mut libarch::interrupts::ControlFlowContext,
        arch_context: &mut libarch::interrupts::ArchContext,
    ) {
        use libarch::interrupts::Vector;

        match Vector::try_from(irq_vector) {
            Ok(vector) if vector == Vector::Timer => {
                crate::local_state::with_scheduler(|scheduler| scheduler.next_task(ctrl_flow_context, arch_context));
            }

            vector_result => {
                warn!("Unhandled IRQ vector: {:?}", vector_result);
            }
        }

        #[cfg(target_arch = "x86_64")]
        libarch::x64::structures::apic::end_of_interrupt();
    }
}
