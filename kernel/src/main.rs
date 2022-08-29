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
    clippy::wildcard_imports
)]
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
    asm_const,
    const_ptr_offset_from,
    const_refs_to_cell,
    exclusive_range_pattern,
    raw_vec_internals,
    allocator_api,
    strict_provenance,
    slice_ptr_get,
    new_uninit,
    inline_const,
    sync_unsafe_cell,
    if_let_guard,
    pointer_is_aligned,
    core_intrinsics,
    panic_info_message,
    alloc_error_handler
)]

use core::sync::atomic::Ordering;

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod arch;
mod boot;
mod drivers;
mod interrupts;
mod local_state;
mod memory;
mod num;
mod scheduling;
mod stdout;
mod tables;
mod time;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO impl stack unwinding

    error!("KERNEL PANIC (at {}): {}", info.location().unwrap(), info.message().unwrap());

    // TODO should we actually abort on every panic?
    crate::interrupts::wait_loop()
}

#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    // TODO should we actually abort on every alloc error?
    crate::interrupts::wait_loop()
}

lazy_static::lazy_static! {
    /// We must take care not to call any allocating functions, or reference KMALLOC itself,
    /// prior to initializing memory (frame/page manager). The SLOB *immtediately* configures
    /// its own allocation table, utilizing both of the aforementioned managers.
    /// TODO this shouldn't be lazy init. Definitely manual init.
    pub static ref KMALLOC: crate::memory::slob::SLOB<'static> = unsafe { crate::memory::slob::SLOB::new() };
}

pub const LIMINE_REV: u64 = 0;
static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);
static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV).flags(0b1);

static CON_OUT: core::cell::SyncUnsafeCell<crate::memory::io::Serial> =
    core::cell::SyncUnsafeCell::new(crate::memory::io::Serial::new(crate::memory::io::COM1));
static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static SMP_MEMORY_INIT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

static mut KERNEL_CFG_SMP: bool = false;

#[no_mangle]
#[allow(clippy::too_many_lines)]
unsafe extern "C" fn _entry() -> ! {
    /* standard output setup */
    {
        let con_out_mut = &mut *CON_OUT.get();
        con_out_mut.init(crate::memory::io::SerialSpeed::S115200);
        crate::stdout::set_stdout(con_out_mut, log::LevelFilter::Debug);
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
            info!("Vendor              {}", vendor_str);
        } else {
            info!("Vendor              None");
        }
    }

    /* parse kernel arguments */
    {
        let kernel_file = LIMINE_KERNEL_FILE
            .get_response()
            .get()
            .expect("bootloader did not provide a kernel file")
            .kernel_file
            .get()
            .expect("bootloader kernel file response did not provide a valid file handle");
        let cmdline = core::ffi::CStr::from_ptr(kernel_file.cmdline.as_mut_ptr().unwrap().cast())
            .to_str()
            .expect("invalid cmdline string");

        for argument in cmdline.split(' ') {
            match argument.split_once(':') {
                Some(("smp", "on")) => KERNEL_CFG_SMP = true,
                Some(("smp", "off")) => KERNEL_CFG_SMP = false,
                _ => warn!("Unhandled cmdline parameter: {:?}", argument),
            }
        }
    }

    crate::memory::init_kernel_hhdm_address();
    crate::memory::init_kernel_frame_manager();
    crate::memory::init_kernel_page_manager();

    /* bsp core init */
    {
        #[cfg(target_arch = "x86_64")]
        {
            crate::arch::x64::cpu::load_registers();
            crate::arch::x64::cpu::load_tables();
        }

        // TODO rv64 bsp hart init
    }

    let frame_manager = crate::memory::get_kernel_frame_manager();
    let page_manager = crate::memory::get_kernel_page_manager();

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
                        frame_manager,
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
                        frame_manager,
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
                        frame_manager,
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
                        frame_manager,
                    )
                    .unwrap();
            });

        frame_manager.iter().enumerate().for_each(|(frame_index, (_, ty))| {
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
                    frame_manager,
                )
                .unwrap();
        });
    }

    /* SMP init */
    // Because the SMP information structures (and thus, their `goto_address`) are only mapped in the bootloader
    // page tables, we have to start the other cores and pass the root page table frame index in. All of the cores
    // will then wait until every core has swapped to the new page tables, then this core (the boot core) will
    // reclaim bootloader memory.
    {
        debug!("Attempting to start additional cores...");

        let smp_response =
            LIMINE_SMP.get_response().as_mut_ptr().expect("bootloader provided no SMP information").as_mut().unwrap();

        if let Some(cpus) = smp_response.cpus() {
            debug!("Detected {} APs.", cpus.len() - 1);

            for cpu_info in cpus {
                if cpu_info.lapic_id != smp_response.bsp_lapic_id {
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
    }

    /* memory finalize */
    {
        debug!("Switching to kernel page tables...");
        page_manager.write_cr3();
        debug!("Kernel has finalized control of page tables.");
        debug!("Assigning global allocator...");
        crate::memory::set_global_allocator(&*crate::KMALLOC);

        #[cfg(target_arch = "x86_64")]
        {
            debug!("Initializing APIC interface...");
            crate::arch::x64::structures::apic::init_interface(frame_manager, page_manager);
        }

        debug!("Initializing ACPI interface...");
        crate::tables::acpi::init_interface();
        debug!("Initializing PCI devices...");
        crate::memory::io::pci::init_pcie_devices();

        debug!("Boot core will release other cores, and then wait for all cores to update root page table.");
        SMP_MEMORY_READY.store(true, Ordering::Relaxed);

        while SMP_MEMORY_INIT.load(Ordering::Relaxed) > 0 {
            core::hint::spin_loop();
        }

        debug!("Reclaiming bootloader reclaimable memory...");
        crate::memory::reclaim_bootloader_frames();
    }

    debug!("Finished initial kernel setup.");

    configure_acpi()
}

unsafe fn configure_acpi() -> ! {
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
    }

    /* enable ACPI SCI interrupts */
    {
        use crate::tables::acpi::PM1a_EVT_BLK;

        // TODO clean this filthy mess up

        let pm1a_evt_blk =
            &crate::tables::acpi::get_fadt().pm1a_event_block().expect("no `PM1a_EVT_BLK` found in FADT");
        let mut pm1a_evt_blk_reg = crate::tables::acpi::Register::<PM1a_EVT_BLK>::new(pm1a_evt_blk)
            .expect("failed to get register for `PM1a_EVT_BLK`");

        let mut reg = crate::tables::acpi::Register::<u16>::IO(crate::memory::io::ReadWritePort::new(
            (pm1a_evt_blk.address + ((pm1a_evt_blk.bit_width / 8) as u64)) as u16,
        ));

        reg.write((1 << 8) | (1 << 0));
    }

    debug!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, Ordering::Relaxed);
    kernel_thread_setup()
}

/// Entrypoint for AP processors.
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

fn syscall_test() -> ! {
    use libkernel::syscall;
    let control = syscall::Control { id: syscall::ID::Test, blah: 0xD3ADC0D3 };
    // TODO don't construct a new timer for every instance, use local timer
    let clock = crate::time::clock::get();

    loop {
        // SAFETY: Temporary syscall test.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let result: u64;

            core::arch::asm!(
                "int {}",
                const crate::interrupts::Vector::Syscall as u8,
                in("rdi") &raw const control,
                out("rsi") result
            );

            info!("{:#X}", result);
        }

        clock.spin_wait_us(500000);
    }
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn kernel_thread_setup() -> ! {
    crate::local_state::init(0);

    // use crate::registers::x64::RFlags;
    use crate::{local_state::try_push_task, scheduling::*};

    try_push_task(Task::new(
        TaskPriority::new(3).unwrap(),
        syscall_test,
        &TaskStackOption::Pages(1),
        {
            #[cfg(target_arch = "x86_64")]
            {
                (
                    crate::arch::x64::cpu::GeneralContext::empty(),
                    crate::arch::x64::cpu::SpecialContext::with_kernel_segments(
                        crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
                    ),
                )
            }
        },
        crate::memory::RootPageTable::read(),
    ))
    .unwrap();

    trace!("Beginning scheduling...");
    crate::local_state::try_begin_scheduling();
    crate::interrupts::wait_loop()
}
