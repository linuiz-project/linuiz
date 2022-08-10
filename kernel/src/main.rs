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
    sync_unsafe_cell
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

/// SAFETY: This function invariantly assumes it will be called only once per core.
#[inline(never)]
pub(self) unsafe fn cpu_setup(is_bsp: bool) -> ! {
    if is_bsp {
        debug!("Configuring global wall clock.");
        crate::clock::configure_and_enable();
    }

    crate::local_state::init(is_bsp);

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
