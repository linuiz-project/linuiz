#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, MemoryDescriptor, MemoryType, Status};
use gsai::structures::memory::PAGE_SIZE;

entrypoint!(kernel_main);
extern "win64" fn kernel_main(boot_info: BootInfo) -> Status {
    if let Err(error) =
        gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug)
    {
        panic!("{}", error);
    }

    info!("Successfully loaded into kernel, with logging enabled.");

    info!("Initializing memory (map, page tables, etc.).");
    init_memory(boot_info.memory_map());
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();

    loop {}

    Status::SUCCESS
}

fn init_memory(memory_map: &[MemoryDescriptor]) {
    info!(
        "Identified {} MB of system memory.",
        gsai::structures::memory::to_mibibytes(
            memory_map
                .iter()
                .map(|descriptor| { descriptor.page_count * PAGE_SIZE })
                .sum()
        )
    );

    let valid_descriptors = memory_map
        .iter()
        .filter(|descriptor| descriptor.ty == efi_boot::KERNEL_CODE);
    info!(
        "Found {} persistable memory descriptors (kernel code, data, etc.).",
        valid_descriptors.count()
    );
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
