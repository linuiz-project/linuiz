#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, MemoryDescriptor, Status};
use gsai::structures::memory::paging::PageFrameAllocator;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(mut boot_info: BootInfo) -> Status {
    if let Err(error) =
        gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug)
    {
        panic!("{}", error);
    } else {
        info!("Successfully loaded into kernel, with logging enabled.");
    }

    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();
    info!(
        "{}",
        boot_info.framebuffer_pointer().unwrap().pointer as u64
    );

    //info!("{}", boot_info.memory_map().len());
    for descriptor in boot_info.memory_map().take(6) {
        info!("{:#?}", descriptor);
        //memory_map[index] = *descriptor;
    }

    info!("Initializing memory (map, page tables, etc.).");
    //init_memory(boot_info.memory_map());

    Status::SUCCESS
}

fn init_memory(memory_map: &[MemoryDescriptor]) {
    let _page_frame_allocator = PageFrameAllocator::from_mmap(memory_map);
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
