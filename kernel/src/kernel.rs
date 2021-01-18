#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, MemoryType, ResetType, Status};
use gsai::structures::memory::{global_allocator, paging::PageTableManager, Frame};

entrypoint!(kernel_main);
extern "win64" fn kernel_main(mut boot_info: BootInfo) -> Status {
    match gsai::logging::init(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug) {
        Ok(()) => info!("Successfully loaded into kernel, with logging enabled."),
        Err(error) => panic!("{}", error),
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();

    info!("Initializing memory (map, page tables, et al).");
    unsafe { gsai::structures::memory::init_global_allocator(boot_info.memory_map()) };
    let mut page_table_manager = PageTableManager::new();
    info!("Identity mapping all utilized addresses to kernel page table manager.");
    for descriptor in boot_info.memory_map() {
        if descriptor.ty != MemoryType::CONVENTIONAL {
            let phys_end = descriptor.phys_start + (descriptor.page_count * 0x1000);
            for frame in Frame::range(descriptor.phys_start..phys_end) {
                page_table_manager.identity_map(&frame);
            }
        }
    }

    // let total_memory = global_allocator(|allocator| allocator.total_memory()) as u64;
    // Frame::range(0x0..total_memory).for_each(|frame| page_table_manager.identity_map(&frame));
    page_table_manager.write_pml4();

    let ret_status = Status::SUCCESS;
    info!("Exiting with status code: {:?}", ret_status);
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

    x86_64::instructions::interrupts::enable();
    info!("(WARN: interrupts are now enabled)");
}
