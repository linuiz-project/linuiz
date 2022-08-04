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
    inline_const
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod boot;
mod clock;
mod drivers;
mod interrupts;
mod local_state;
mod logging;
mod memory;
mod scheduling;
mod tables;

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

pub unsafe fn cpu_setup(is_bsp: bool) -> ! {
    /* load registers */
    #[cfg(target_arch = "x86_64")]
    {
        // Set CR0 flags.
        use libkernel::registers::x64::control::{CR0Flags, CR0};
        CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG);

        // Set CR4 flags.
        use libkernel::{
            cpu::x64::{EXT_FEATURE_INFO, FEATURE_INFO},
            registers::x64::control::{CR4Flags, CR4},
        };

        let mut flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

        if FEATURE_INFO.as_ref().map(|info| info.has_de()).unwrap_or(false) {
            trace!("Detected support for debugging extensions.");
            flags.insert(CR4Flags::DE);
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_fxsave_fxstor()).unwrap_or(false) {
            trace!("Detected support for `fxsave` and `fxstor` instructions.");
            flags.insert(CR4Flags::OSFXSR);
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_mce()).unwrap_or(false) {
            trace!("Detected support for machine check exceptions.")
        }

        if FEATURE_INFO.as_ref().map(|info| info.has_pcid()).unwrap_or(false) {
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

        // TODO research SMAP/SMEP
        // if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smep()).unwrap_or(false) {
        //     trace!("Detected support for supervisor mode execution prevention.");
        //     flags.insert(CR4Flags::SMEP);
        // }

        // if EXT_FEATURE_INFO.as_ref().map(|info| info.has_smap()).unwrap_or(false) {
        //     trace!("Detected support for supervisor mode access prevention.");
        //     flags.insert(CR4Flags::SMAP);
        // }

        CR4::write(flags);

        // Enable use of the `NO_EXECUTE` page attribute, if supported.
        if libkernel::cpu::x64::EXT_FUNCTION_INFO
            .as_ref()
            .map(|func_info| func_info.has_execute_disable())
            .unwrap_or(false)
        {
            libkernel::registers::x64::msr::IA32_EFER::set_nxe(true);
        } else {
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
        }
    }

    /* load tables */
    {
        trace!("Configuring local tables (IDT, GDT).");

        // Always initialize GDT prior to configuring IDT.
        tables::gdt::init();

        if is_bsp {
            // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
            // it must be ensured that the handlers are set only *after* the GDT has been
            // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
            // is incorrect, and this causes very confusing GPFs.
            interrupts::init_idt();

            fn apit_empty(
                _: &mut x86_64::structures::idt::InterruptStackFrame,
                _: &mut crate::scheduling::ThreadRegisters,
            ) {
                libkernel::structures::apic::end_of_interrupt();
            }

            interrupts::set_handler_fn(interrupts::Vector::LINT0_VECTOR, apit_empty);
            interrupts::set_handler_fn(interrupts::Vector::LINT1_VECTOR, apit_empty);
            interrupts::set_handler_fn(interrupts::Vector::Syscall, crate::interrupts::syscall::handler);
        }

        interrupts::load_idt();
    }

    if is_bsp {
        debug!("Configuring global wall clock.");
        crate::clock::configure_and_enable();
    }

    local_state::init();

    /* load tss */
    {
        use x86_64::{
            instructions::tables,
            structures::{
                gdt::{Descriptor, GlobalDescriptorTable},
                tss::TaskStateSegment,
            },
        };

        trace!("Configuring new TSS and loading via temp GDT.");

        let tss_ptr = alloc::boxed::Box::leak(crate::local_state::generate_tss().unwrap()) as *mut TaskStateSegment;

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

        trace!("TSS loaded.");
    }

    trace!("Core-local setup complete. Running kernel thread.");
    run_kernel(is_bsp)
}

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
                "int 0x80",
                in("rdi") &raw const control,
                out("rsi") result
            );
        }

        info!("{:#X}", result);

        clock::busy_wait_msec(500);
    }
}

#[inline(never)]
unsafe fn run_kernel(_is_bsp: bool) -> ! {
    //if is_bsp {
    use crate::{local_state::try_push_task, scheduling::*};
    use libkernel::registers::x64::RFlags;

    try_push_task(Task::new(
        TaskPriority::new(3).unwrap(),
        syscall_test,
        TaskStackOption::Pages(1),
        RFlags::INTERRUPT_FLAG,
        *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
        *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
        libkernel::registers::x64::control::CR3::read(),
    ))
    .unwrap();

    // Add a number of test tasks to get kernel output, test scheduling, and test logging.
    for _ in 0..1 {
        // try_push_task(Task::new(
        //     TaskPriority::new(1).unwrap(),
        //     logging_test,
        //     TaskStackOption::Pages(1),
        //     RFlags::INTERRUPT_FLAG,
        //     *crate::taregisters::x64SELECTOR.get().unwrap(),
        //     *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
        //     libkernel::registers::x86_64::control::CR3::read(),
        // ))
        // .unwrap();
    }

    crate::local_state::try_begin_scheduling();
    libkernel::instructions::interrupts::wait_indefinite()

    /* ENABLE SYSCALL */
    // {
    //     use crate::tables::gdt;
    //     use libkernel::registers::msr;

    //     // Configure system call environment registers.
    //     msr::IA32_STAR::set_selectors(
    //         *gdt::KCODE_SELECTOR.get().unwrap(),
    //         *gdt::KDATA_SELECTOR.get().unwrap(),
    //     );
    //     msr::IA32_LSTAR::set_syscall(syscall::syscall_enter);
    //     msr::IA32_SFMASK::set_rflags_mask(libkernel::registers::RFlags::all());
    //     // Enable `syscall`/`sysret`.
    //     msr::IA32_EFER::set_sce(true);
    // }

    // libkernel::registers::stack::RSP::write(libkernel::memory::alloc_stack(1, true));
    // libkernel::cpu::ring3_enter(test_user_function, libkernel::registers::RFlags::empty());
}
