#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;

use efi_boot::{entrypoint, BootInfo, ResetType, Status};
use gsai::memory::{
    allocators::{global_memory, total_memory_iter},
    paging::{MappedVirtualAddessor, VirtualAddessor},
    Frame,
};
use x86_64::{PhysAddr, VirtAddr};

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
    unsafe { gsai::memory::allocators::init_global_memory(boot_info.memory_map()) };
    // TODO possibly retrieve base address using min_by_key so it's accurate and not a guess
    // motivation: is the base address of RAM ever not 0x0?
    let mut virtual_addressor = unsafe { MappedVirtualAddessor::new(VirtAddr::zero()) };
    debug!("Identity mapping all available memory.");
    total_memory_iter().step_by(0x1000).for_each(|addr| {
        virtual_addressor.identity_map(&Frame::from_addr(PhysAddr::new(addr as u64)))
    });
    virtual_addressor
        .modify_mapped_addr(global_memory(|allocator| allocator.physical_mapping_addr()));
    virtual_addressor.swap_into();

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
