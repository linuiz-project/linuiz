#![no_std]
#![no_main]
#![feature(asm, abi_efiapi)]

#[macro_use]
extern crate log;
extern crate alloc;

mod drivers;
mod logging;
mod timer;

use core::ffi::c_void;
use libkernel::{
    memory::{paging::VirtualAddressor, UEFIMemoryDescriptor},
    structures, BootInfo, ConfigTableEntry, VirtAddr,
};
use structures::acpi::{Checksum, RDSPDescriptor2, SDTHeader};

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
    log::LevelFilter::Trace
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

    let entry = boot_info
        .config_table()
        .iter()
        .find(|entry| entry.guid() == libkernel::structures::ACPI2_GUID)
        .unwrap();
    let rdsp: &RDSPDescriptor2 = unsafe { &*(entry.addr().as_u64() as *const _) };
    info!("{:?}", rdsp.checksum());
    let xsdt: &SDTHeader = unsafe { &*(rdsp.addr().as_u64() as *const _) };
    info!(
        "{} {} {}, {:?}",
        xsdt.signature(),
        xsdt.oem_id(),
        xsdt.oem_table_id(),
        xsdt.checksum()
    );

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");
    libkernel::structures::pic::init();
    info!("Successfully initialized PIC.");

    let memory_map = boot_info.memory_map();
    info!("Initializing global memory (frame allocator, global allocator, et al).");
    // let frame_map_frames = unsafe { libkernel::memory::init_global_memory(memory_map) };
    // let mut global_addressor = init_global_addressor(memory_map);
    // frame_map_frames.for_each(|frame| global_addressor.identity_map(&frame));
    // debug!("Setting global addressor (`alloc::*` will be usable after this point).");
    // unsafe { libkernel::memory::set_global_addressor(global_addressor) };
    unsafe { libkernel::memory::init_global_allocator(memory_map) };

    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    let mut slice = alloc::vec::Vec::<u8>::new();
    let mut slice2 = alloc::vec::Vec::<u8>::new();
    for index in 0..50 {
        slice.push(index);
        slice2.push(index);
    }

    info!("{:?}\n{:?}", slice, slice2);

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}
