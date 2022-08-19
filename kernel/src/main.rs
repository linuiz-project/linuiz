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
    pointer_is_aligned
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod drivers;
mod interrupts;
mod local_state;
mod logging;
mod memory;
mod scheduling;
mod tables;
mod time;

lazy_static::lazy_static! {
    /// We must take care not to call any allocating functions, or reference KMALLOC itself,
    /// prior to initializing memory (frame/page manager). The SLOB *immtediately* configures
    /// its own allocation table, utilizing both of the aforementioned managers.
    pub static ref KMALLOC: memory::SLOB<'static> = unsafe { memory::SLOB::new() };
}

pub const LIMINE_REV: u64 = 0;
static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV);

static mut CON_OUT: crate::drivers::stdout::Serial = crate::drivers::stdout::Serial::new(crate::drivers::stdout::COM1);
static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static SMP_LAPIC_IDS: spin::Once<crossbeam_queue::ArrayQueue<u32>> = spin::Once::new();

#[no_mangle]
unsafe extern "sysv64" fn _entry() -> ! {
    CON_OUT.init(crate::drivers::stdout::SerialSpeed::S115200);
    match crate::drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Trace) {
        Ok(()) => info!("Successfully loaded into kernel."),
        Err(_) => libkernel::instructions::interrupts::wait_indefinite(),
    }

    let boot_info = LIMINE_INFO.get_response().get().expect("bootloader provided no info");
    info!(
        "Bootloader Info     {} v{} (rev {})",
        core::ffi::CStr::from_ptr(boot_info.name.as_ptr().unwrap() as *const _).to_str().unwrap(),
        core::ffi::CStr::from_ptr(boot_info.version.as_ptr().unwrap() as *const _).to_str().unwrap(),
        boot_info.revision,
    );

    if let Some(vendor_str) = libkernel::cpu::get_vendor() {
        info!("Vendor              {}", vendor_str);
    } else {
        info!("Vendor              None");
    }

    load_registers();
    debug!("Initializing kernel page manager...");
    crate::memory::init_kernel_page_manager();
    load_tables();
    debug!("Switch to kernel page tables...");
    crate::memory::get_kernel_page_manager().write_cr3();
    info!("worked");
    crate::memory::reclaim_bootloader_memory();
    trace!("Assigning libkernel global allocator.");
    libkernel::memory::global_alloc::set(&*crate::KMALLOC);

    debug!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, core::sync::atomic::Ordering::Relaxed);
    core_setup(true)
    // configure_acpi()
}

fn load_registers() {
    trace!("Loading x86-specific control registers to known state.");

    // Set CR0 flags.
    use libkernel::registers::control::{CR0Flags, CR0};
    unsafe { CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG) };

    // Set CR4 flags.
    use libkernel::{
        cpu::{EXT_FEATURE_INFO, FEATURE_INFO},
        registers::control::{CR4Flags, CR4},
    };

    let mut flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

    if FEATURE_INFO.has_de() {
        trace!("Detected support for debugging extensions.");
        flags.insert(CR4Flags::DE);
    }

    if FEATURE_INFO.has_fxsave_fxstor() {
        trace!("Detected support for `fxsave` and `fxstor` instructions.");
        flags.insert(CR4Flags::OSFXSR);
    }

    if FEATURE_INFO.has_mce() {
        trace!("Detected support for machine check exceptions.")
    }

    if FEATURE_INFO.has_pcid() {
        trace!("Detected support for process context IDs.");
        flags.insert(CR4Flags::PCIDE);
    }

    if EXT_FEATURE_INFO.as_ref().map(|info| info.has_umip()).unwrap_or(false) {
        trace!("Detected support for usermode instruction prevention.");
        flags.insert(CR4Flags::UMIP);
    }

    if EXT_FEATURE_INFO.as_ref().map(|info| info.has_fsgsbase()).unwrap_or(false) {
        trace!("Detected support for CPL3 FS/GS base usage.");
        flags.insert(CR4Flags::FSGSBASE);
    }

    if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smep()).unwrap_or(false) {
        trace!("Detected support for supervisor mode execution prevention.");
        flags.insert(CR4Flags::SMEP);
    }

    if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smap()).unwrap_or(false) {
        trace!("Detected support for supervisor mode access prevention.");
        flags.insert(CR4Flags::SMAP);
    }

    unsafe { CR4::write(flags) };

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if libkernel::cpu::EXT_FUNCTION_INFO.as_ref().map(|func_info| func_info.has_execute_disable()).unwrap_or(false) {
        trace!("Detected support for paging execution prevention.");
        unsafe { libkernel::registers::msr::IA32_EFER::set_nxe(true) };
    } else {
        warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
    }
}

/// SAFETY: Caller must ensure this method is called only once per core.
unsafe fn load_tables() {
    trace!("Configuring local tables (IDT, GDT).");

    // Always initialize GDT prior to configuring IDT.
    crate::tables::gdt::init();

    // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
    // it must be ensured that the handlers are set only *after* the GDT has been
    // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
    // is incorrect, and this causes very confusing GPFs.
    let idt_frame_index = crate::memory::get_kernel_frame_manager().lock_next().unwrap();
    let idt_addr = crate::memory::get_kernel_hhdm_addr().as_usize() + (idt_frame_index * 0x1000);
    let idt = &mut *(idt_addr as *mut x86_64::structures::idt::InterruptDescriptorTable);
    crate::interrupts::set_exception_handlers(idt);
    crate::interrupts::set_stub_handlers(idt);
    crate::tables::idt::store(idt);
}

unsafe fn configure_acpi() -> ! {
    /* prepare APs for startup */
    // TODO add a kernel parameter for SMP
    let smp_count = {
        let smp_response =
            LIMINE_SMP.get_response().as_mut_ptr().expect("received no SMP response from bootloader").as_mut().unwrap();

        let lapic_ids = SMP_LAPIC_IDS.call_once(|| crossbeam_queue::ArrayQueue::new(smp_response.cpu_count as usize));

        if let Some(cpus) = smp_response.cpus() {
            debug!("Detected {} APs.", cpus.len() - 1);

            for cpu_info in cpus {
                // Ensure we don't try to 'start' the BSP.
                if cpu_info.lapic_id != smp_response.bsp_lapic_id {
                    debug!("Starting processor: PID{}/LID{}", cpu_info.processor_id, cpu_info.lapic_id);
                    cpu_info.goto_address = _smp_entry as u64;
                    lapic_ids.push(cpu_info.lapic_id).expect("LAPIC ID queue unexpectedly full")
                }
            }
        }

        smp_response.cpu_count as u32
    };

    /* configure I/O APIC redirections */
    {
        let mut cur_vector = 0x30;
        let ioapics = crate::interrupts::ioapic::get_io_apics();

        for irq_source in &crate::tables::acpi::get_apic_model().interrupt_source_overrides {
            debug!("Processing interrupt source override: {:?}", irq_source);

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
            redirection.set_destination_id(libkernel::cpu::get_id() as u8);
            target_ioapic.set_redirection(irq_source.global_system_interrupt, redirection);
        }

        for nmi_source in &crate::tables::acpi::get_apic_model().nmi_sources {
            debug!("Processing NMI source override: {:?}", nmi_source);

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
            redirection.set_destination_id(libkernel::cpu::get_id() as u8);
            target_ioapic.set_redirection(nmi_source.global_system_interrupt, redirection);
        }
    }

    /* configure AML handling */
    {
        let _aml_context = crate::tables::acpi::get_aml_context();
    }

    /* take ownership of ACPI */
    {
        use acpi::platform::address::AddressSpace;
        use bit_field::BitField;

        let fadt = tables::acpi::get_fadt();

        let mut smi_cmd = libkernel::io::port::WriteOnlyPort::<u8>::new(fadt.smi_cmd_port as u16);
        smi_cmd.write(fadt.acpi_enable);

        {
            let pm1a_cnt_blk_reg = crate::tables::acpi::Register::<u16>::new(
                &fadt.pm1a_control_block().expect("no `PM1a_CNT_BLK` found in FADT"),
            )
            .expect("failed to get register for `PM1a_CNT_BLK`");

            while !pm1a_cnt_blk_reg.read().get_bit(0) {
                libkernel::instructions::pause();
            }
        }

        // Enable relevant bits for ACPI SCI interrupt triggers.
        {
            let pm1a_evt_blk = &fadt.pm1a_event_block().expect("no `PM1a_EVT_BLK` found in FADT");
            let mut pm1a_evt_blk_reg = crate::tables::acpi::Register::<u16>::new(pm1a_evt_blk)
                .expect("failed to get register for `PM1a_EVT_BLK`");

            info!("{:#?}", pm1a_evt_blk);
            info!("{:#b}", pm1a_evt_blk_reg.read());
            info!("{:#b}", pm1a_evt_blk_reg.read());
            pm1a_evt_blk_reg.write(0b01);
            // pm1a_evt_blk_reg.write(*(pm1a_evt_blk_reg.read().set_bit(0, true).set_bit(8, true).set_bit(9, true)));
            info!("{:#b}", pm1a_evt_blk_reg.read());
            info!("{:#b}", pm1a_evt_blk_reg.read());
        }
    }

    debug!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, core::sync::atomic::Ordering::Relaxed);
    core_setup(true)
}

/// Entrypoint for AP processors.
#[inline(never)]
unsafe extern "C" fn _smp_entry() -> ! {
    // Wait to ensure the machine is the correct state to execute cpu setup.
    while !SMP_MEMORY_READY.load(core::sync::atomic::Ordering::Relaxed) {}

    load_registers();
    load_tables();

    core_setup(false)
}

/// SAFETY: This function invariantly assumes it will only be called once.
unsafe fn core_setup(is_bsp: bool) -> ! {
    /* load tss */
    {
        use crate::interrupts::StackTableIndex;
        use alloc::boxed::Box;
        use libkernel::memory::{page_aligned_allocator, PageAlignedBox};
        use x86_64::{
            instructions::tables,
            structures::{
                gdt::{Descriptor, GlobalDescriptorTable},
                tss::TaskStateSegment,
            },
            VirtAddr,
        };

        const PRIVILEGE_STACK_SIZE: usize = 0x5000;
        const EXCEPTION_STACK_SIZE: usize = 0x2000;

        let privilege_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(PRIVILEGE_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(PRIVILEGE_STACK_SIZE);
        let db_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let nmi_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let df_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);
        let mc_stack_ptr =
            Box::leak(PageAlignedBox::<[u8]>::new_uninit_slice_in(EXCEPTION_STACK_SIZE, page_aligned_allocator()))
                .as_mut_ptr()
                .add(EXCEPTION_STACK_SIZE);

        trace!("Configuring new TSS and loading via temp GDT.");

        let tss_ptr = Box::leak({
            let mut tss = Box::new(x86_64::structures::tss::TaskStateSegment::new());

            tss.privilege_stack_table[0] = VirtAddr::from_ptr(privilege_stack_ptr);
            tss.interrupt_stack_table[StackTableIndex::Debug as usize] = VirtAddr::from_ptr(db_stack_ptr);
            tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] = VirtAddr::from_ptr(nmi_stack_ptr);
            tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] = VirtAddr::from_ptr(df_stack_ptr);
            tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] = VirtAddr::from_ptr(mc_stack_ptr);

            tss
        }) as *mut _;

        trace!("Configuring TSS descriptor for temp GDT.");
        let tss_descriptor = {
            use bit_field::BitField;

            let tss_ptr_u64 = tss_ptr as u64;

            let mut low = x86_64::structures::gdt::DescriptorFlags::PRESENT.bits();
            // base
            low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
            low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
            // limit (the `-1` is needed since the bound is inclusive, not exclusive)
            low.set_bits(0..16, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);
            // type (0b1001 = available 64-bit tss)
            low.set_bits(40..44, 0b1001);

            // high 32 bits of base
            let mut high = 0;
            high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

            Descriptor::SystemSegment(low, high)
        };

        trace!("Loading in temp GDT to `ltr` the TSS.");
        // Store current GDT pointer to restore later.
        let cur_gdt = tables::sgdt();
        // Create temporary kernel GDT to avoid a GPF on switching to it.
        let mut temp_gdt = GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);

        // Load temp GDT ...
        temp_gdt.load_unsafe();
        // ... load TSS from temporary GDT ...
        tables::load_tss(tss_selector);
        // ... and restore cached GDT.
        tables::lgdt(&cur_gdt);

        trace!("TSS loaded, and temporary GDT trashed.");
    }

    trace!("Arch-specific local setup complete.");
    crate::cpu_setup(is_bsp)
}

// use libkernel::io::pci;
// pub struct Devices<'a>(Vec<pci::DeviceVariant>, &'a core::marker::PhantomData<()>);
// unsafe impl Send for Devices<'_> {}
// unsafe impl Sync for Devices<'_> {}

// impl Devices<'_> {
//     pub fn iter(&self) -> core::slice::Iter<pci::DeviceVariant> {
//         self.0.iter()
//     }
// }

// This might need to be in `libkernel`? Or some.. more semantic access method
// lazy_static::lazy_static! {
//     pub static ref PCIE_DEVICES: Devices<'static> =
//         Devices(
//             libkernel::io::pci::get_pcie_devices().collect(),
//             &core::marker::PhantomData
//         );
// }

// extern "C" fn new_nvme_handler(device_index: usize) -> ! {
//     if let libkernel::io::pci::DeviceVariant::Standard(pcie_device) = &PCIE_DEVICES.0[device_index]
//     {
//         let nvme_controller =
//             drivers::nvme::Controller::from_device_and_configure(pcie_device, 8, 8);

//         use drivers::nvme::command::admin::*;
//         nvme_controller.submit_admin_command(AdminCommand::Identify { ctrl_id: 0 });
//         nvme_controller.run()
//     } else {
//         error!(
//             "Given PCI device index was invalid for NVMe controller (index {}).",
//             device_index
//         );
//     }

//     libkernel::instructions::hlt_indefinite()
// }

// fn logging_test() -> ! {
//     loop {
//         info!("TEST");
//         clock::busy_wait_msec(500);
//     }
// }

fn syscall_test() -> ! {
    use libkernel::syscall;
    let control = syscall::Control { id: syscall::ID::Test, blah: 0xD3ADC0D3 };
    // TODO don't construct a new timer for every instance, use local timer
    let clock = crate::time::clock::get();

    loop {
        let result: u64;

        unsafe {
            core::arch::asm!(
                "int 0xF0",
                in("rdi") &raw const control,
                out("rsi") result
            );
        }

        info!("{:#X}", result);

        clock.spin_wait_us(500000)
    }
}

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn cpu_setup(is_bsp: bool) -> ! {
    crate::local_state::init(is_bsp);

    use crate::{local_state::try_push_task, scheduling::*};
    use libkernel::registers::RFlags;

    try_push_task(Task::new(
        TaskPriority::new(3).unwrap(),
        syscall_test,
        TaskStackOption::Pages(1),
        RFlags::INTERRUPT_FLAG,
        *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
        *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
        libkernel::registers::control::CR3::read(),
    ))
    .unwrap();

    crate::local_state::try_begin_scheduling();
    libkernel::instructions::interrupts::wait_indefinite()
}
