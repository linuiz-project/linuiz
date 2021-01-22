#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;
extern crate alloc;

use efi_boot::BootInfo;
use gsai::memory::{
    allocators::{global_memory, init_global_allocator},
    paging::VirtualAddressorCell,
    Frame, UEFIMemoryDescriptor,
};

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Info
}

static KERNEL_ADDRESSOR: VirtualAddressorCell = VirtualAddressorCell::empty();

#[export_name = "_start"]
extern "win64" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor>) -> ! {
    match gsai::logging::init_logger(gsai::logging::LoggingModes::SERIAL, get_log_level()) {
        Ok(()) => info!("Successfully loaded into kernel, with logging enabled."),
        Err(error) => panic!("{}", error),
    }
    //info!("{:?}", unsafe { &_text_start as *const _ as *const char });
    // boot_info
    //     .memory_map()
    //     .iter()
    //     .filter(|descriptor| descriptor.ty == MemoryType::BOOT_SERVICES_DATA)
    //     .for_each(|descriptor| debug!("{:#?}", descriptor));

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();

    info!("Initializing global memory (physical frame allocator / journal).");
    unsafe { gsai::memory::allocators::init_global_memory(boot_info.memory_map()) };
    // TODO possibly retrieve base address using min_by_key so it's accurate and not a guess
    // motivation: is the base address of RAM ever not 0x0?

    init_virtual_addressor(boot_info.memory_map());

    //let vec = alloc::vec![0u8; 1000];

    info!("Kernel has reached safe shutdown state.");
    unsafe { gsai::instructions::pwm::qemu_shutdown() }
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
    warn!("Interrupts are now enabled!");
}

fn init_virtual_addressor<'balloc>(memory_map: &[gsai::memory::UEFIMemoryDescriptor]) {
    debug!("Creating virtual addressor for kernel (starting at 0x0, identity-mapped).");
    KERNEL_ADDRESSOR.init(gsai::memory::Page::null());

    for frame in memory_map
        .iter()
        .filter(|descriptor| gsai::memory::is_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| Frame::range_count(descriptor.phys_start, descriptor.page_count))
    {
        KERNEL_ADDRESSOR.identity_map(&frame);
    }

    gsai::linker_statics::validate_section_mappings(&KERNEL_ADDRESSOR);
    //virtual_addressor.modify_mapped_addr(global_memory(|allocator| allocator.physical_mapping_addr()));
    KERNEL_ADDRESSOR.swap_into();
    loop {}
    unsafe { init_global_allocator(&KERNEL_ADDRESSOR) };
}
