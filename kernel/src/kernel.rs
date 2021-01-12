#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, ResetType, Status};
use gsai::structures::memory::{
    paging::{FrameAllocator, PageTableManager},
    Frame,
};
use x86_64::VirtAddr;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(mut boot_info: BootInfo) -> Status {
    match gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Trace) {
        Ok(()) => info!("Successfully loaded into kernel, with logging enabled."),
        Err(error) => panic!("{}", error),
    }

    info!("Validating alignment of BootInfo.");
    boot_info.validate_magic();
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();
    info!("Initializing memory (map, page tables, etc.).");
    let frame_allocator = FrameAllocator::from_mmap(boot_info.memory_map());
    let mut page_table_manager = PageTableManager::new(frame_allocator);
    page_table_manager
        .map_memory(VirtAddr::new(0x2000), &Frame::from_addr(0x1000))
        .ok();
    page_table_manager
        .map_memory(VirtAddr::new(0x3000), &Frame::from_addr(0x9000))
        .ok();

    unsafe { boot_info.runtime_table().runtime_services() }.reset(
        ResetType::Shutdown,
        Status::SUCCESS,
        None,
    )
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

    //x86_64::instructions::interrupts::enable();
    info!("(WARN: interrupts are now enabled)");
}
