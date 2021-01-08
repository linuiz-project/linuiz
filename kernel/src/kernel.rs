#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, MemoryDescriptor, ResetType, Status};
use gsai::structures::memory::paging::PageFrameAllocator;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(mut boot_info: BootInfo) -> Status {
    match gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug) {
        Ok(()) => info!("Successfully loaded into kernel, with logging enabled."),
        Err(error) => panic!("{}", error),
    }

    info!("Validating state of BootInfo.");
    boot_info.validate_magic();
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();
    info!("Initializing memory (map, page tables, etc.).");
    let frame_allocator = init_memory(boot_info.memory_map());

    unsafe { boot_info.runtime_table().runtime_services() }.reset(
        ResetType::Shutdown,
        Status::SUCCESS,
        None,
    )
}

fn init_memory(memory_map: &[MemoryDescriptor]) -> PageFrameAllocator {
    PageFrameAllocator::from_mmap(memory_map)
}

unsafe fn init_cpu_state() {
    gsai::instructions::init_segment_registers(0x0);
    info!("Zeroed segment registers.");
}

fn init_structures() {
    gsai::structures::gdt::init();
    info!("Successfully initialized GDT.");
    gsai::structures::idt::init();
    info!("Successfully initialized IDT.");
    gsai::structures::pic::init();
    info!("Successfully initialized PIC.");

    x86_64::instructions::interrupts::enable();
    info!("(WARN: interrupts are now enabled)");
}
