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
    if_let_guard
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

use libkernel::LinkerSymbol;

extern "C" {
    static __code_start: LinkerSymbol;
    static __startup_start: LinkerSymbol;
    static __code_end: LinkerSymbol;

    static __ro_start: LinkerSymbol;
    static __ro_end: LinkerSymbol;

    static __relro_start: LinkerSymbol;
    static __relro_end: LinkerSymbol;

    static __rw_start: LinkerSymbol;
    static __bsp_top: LinkerSymbol;
    static __rw_end: LinkerSymbol;
}

lazy_static::lazy_static! {
    /// We must take care not to call any allocating functions, or reference KMALLOC itself,
    /// prior to initializing memory (frame/page manager). The SLOB *immtediately* configures
    /// its own allocation table, utilizing both of the aforementioned managers.
    pub static ref KMALLOC: memory::SLOB<'static> = unsafe { memory::SLOB::new() };
}

pub const LIMINE_REV: u64 = 0;
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV);
static LIMINE_INFO: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);

const DEV_UNMAP_LOWER_HALF_IDMAP: bool = false;
static mut CON_OUT: crate::drivers::stdout::Serial = crate::drivers::stdout::Serial::new(crate::drivers::stdout::COM1);
static SMP_MEMORY_READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[no_mangle]
unsafe extern "sysv64" fn _entry() -> ! {
    CON_OUT.init(crate::drivers::stdout::SerialSpeed::S115200);
    match crate::drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Trace) {
        Ok(()) => info!("Successfully loaded into kernel."),
        Err(_) => libkernel::instructions::interrupts::wait_indefinite(),
    }

    /* log boot info */
    {
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
    }

    /* prepare APs for startup */
    // TODO add a kernel parameter for SMP
    {
        let smp_response =
            LIMINE_SMP.get_response().as_mut_ptr().expect("received no SMP response from bootloader").as_mut().unwrap();

        if let Some(cpus) = smp_response.cpus() {
            debug!("Detected {} APs.", cpus.len() - 1);

            for cpu_info in cpus {
                // Ensure we don't try to 'start' the BSP.
                if cpu_info.lapic_id != smp_response.bsp_lapic_id {
                    debug!("Starting processor: PID{}/LID{}", cpu_info.processor_id, cpu_info.lapic_id);
                    cpu_info.goto_address = _smp_entry as u64;
                }
            }
        }
    }

    /* init memory */
    {
        trace!("Configuring kernel memory.");

        // Next, we create the kernel page manager, utilizing the bootloader's higher-half direct
        // mapping for virtual offset mapping.
        let hhdm_addr = crate::memory::get_kernel_hhdm_addr();
        trace!("Higher half identity mapping base: {:?}", hhdm_addr);

        let frame_manager = crate::memory::get_kernel_frame_manager();
        // The frame manager's allocation table is allocated with identity mapping assumed,
        // so before we unmap the lower half virtual memory mapping (for kernel heap), we
        // must ensure the frame manager uses the HHDM base.
        frame_manager.slide_table_base(hhdm_addr.as_usize());

        // if DEV_UNMAP_LOWER_HALF_IDMAP {
        //     let page_manager = crate::memory::get_kernel_page_manager();
        //     trace!("Unmapping lower half identity mappings.");
        //     for entry in memory_map.iter() {
        //         for page in (entry.base..(entry.base + entry.len))
        //             .step_by(0x1000)
        //             .map(|base| Page::from_index((base / 0x1000) as usize))
        //         {
        //             // TODO maybe sometimes this fails? It did before, but isn't now. Could be because of an update to Limine.
        //             page_manager.unmap(&page, libkernel::memory::FrameOwnership::None, frame_manager).unwrap();
        //         }
        //     }
        // }

        // The global kernel allocator must be set AFTER the upper half
        // identity mappings are purged, so that the allocation table
        // (which will reside in the lower half) isn't unmapped.
        trace!("Assigning libkernel global allocator.");
        libkernel::memory::global_alloc::set(&*crate::KMALLOC);
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

    core_setup(false)
}

/// SAFETY: This function invariantly assumes it will only be called once.
unsafe fn core_setup(is_bsp: bool) -> ! {
    /* load registers */
    {
        // Set CR0 flags.
        use libkernel::registers::control::{CR0Flags, CR0};
        CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG);

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

        CR4::write(flags);

        // Enable use of the `NO_EXECUTE` page attribute, if supported.
        if libkernel::cpu::EXT_FUNCTION_INFO.as_ref().map(|func_info| func_info.has_execute_disable()).unwrap_or(false)
        {
            libkernel::registers::msr::IA32_EFER::set_nxe(true);
        } else {
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
        }
    }

    /* load tables */
    {
        trace!("Configuring local tables (IDT, GDT).");

        // Always initialize GDT prior to configuring IDT.
        crate::tables::gdt::init();

        crate::interrupts::set_common_interrupt_handler(crate::interrupts::common_interrupt_handler);
        // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
        // it must be ensured that the handlers are set only *after* the GDT has been
        // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
        // is incorrect, and this causes very confusing GPFs.
        let mut idt = Box::new(x86_64::structures::idt::InterruptDescriptorTable::new());
        crate::interrupts::set_exception_handlers(idt.as_mut());
        crate::interrupts::set_stub_handlers(idt.as_mut());
        crate::tables::idt::store(idt);

        /* load tss */
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

    /* global clock */
    trace!("Loading ACPI timer as global system clock.");
    {
        crate::time::clock::set_system_clock(alloc::boxed::Box::new(
            crate::time::clock::ACPIClock::load().expect("failed to load ACPI timer"),
        ));
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

        use crate::time::clock;
        let wait_window = (clock::get_frequency() / 1000) * 500;
        let target_wait = clock::get_timestamp() + wait_window;
        while clock::get_timestamp() < target_wait {}
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
