#![no_std]
#![no_main]
#![feature(asm, abi_efiapi)]

#[macro_use]
extern crate log;
extern crate alloc;

mod drivers;
mod logging;
mod timer;

use core::{ffi::c_void, str::from_utf8};
use libkernel::{memory::UEFIMemoryDescriptor, structures, BootInfo, ConfigTableEntry, VirtAddr};
use structures::acpi::{Checksum, InterruptDevice, RDSPDescriptor2, SDTHeader, MADT, XSDT};

extern "C" {
    static _text_start: c_void;
    static _text_end: c_void;

    static _rodata_start: c_void;
    static _rodata_end: c_void;

    static _data_start: c_void;
    static _data_end: c_void;

    static _bss_start: c_void;
    static _bss_end: c_void;
}

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Info
}

#[no_mangle]
#[export_name = "_start"]
extern "efiapi" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor, ConfigTableEntry>) -> ! {
    match crate::logging::init_logger(crate::logging::LoggingModes::SERIAL, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
            debug!("Minimum logging level configured as: {:?}", get_log_level());
        }
        Err(error) => panic!("{}", error),
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");
    libkernel::structures::pic::init();
    info!("Successfully initialized PIC.");

    let entry = boot_info
        .config_table()
        .iter()
        .find(|entry| entry.guid() == libkernel::structures::ACPI2_GUID)
        .unwrap();
    let rdsp = unsafe { &*(entry.addr().as_u64() as *const RDSPDescriptor2) };
    let xsdt = rdsp.xsdt();

    for (index, ptr) in xsdt.sdt_ptrs().iter().enumerate() {
        let signature = core::str::from_utf8(unsafe { &*((*ptr) as *const [u8; 4]) }).unwrap();

        if signature == "APIC" {
            let madt = xsdt.index_as::<MADT>(index);
            let madt_iter = madt.iter();

            for (index, interrupt_device) in madt_iter.enumerate() {
                info!("{:?}", interrupt_device);
            }
        }
    }

    loop {}

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    unsafe { libkernel::memory::init_global_allocator(boot_info.memory_map()) };

    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}
