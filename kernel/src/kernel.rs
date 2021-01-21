#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;
extern crate alloc;

use efi_boot::BootInfo;
use gsai::{
    memory::{
        allocators::{global_memory, init_global_allocator, BumpAllocator},
        paging::{VirtualAddressor, VirtualAddressorCell},
        Frame, UEFIMemoryDescriptor,
    },
    structures::runtime_services::RuntimeTable,
};
use x86_64::{PhysAddr, VirtAddr};

static mut VIRTUAL_ADDESSOR: VirtualAddressorCell = VirtualAddressorCell::empty();

#[export_name = "_start"]
extern "win64" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor, RuntimeTable>) -> ! {
    match gsai::logging::init_logger(gsai::logging::LoggingModes::SERIAL, log::LevelFilter::Debug) {
        Ok(()) => info!("Successfully loaded into kernel, with logging enabled."),
        Err(error) => panic!("{}", error),
    }

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

    let bump_allocator = init_virtual_addressor(boot_info.memory_map());
    unsafe { init_global_allocator(bump_allocator) };

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

fn init_virtual_addressor<'balloc>(
    memory_map: &[gsai::memory::UEFIMemoryDescriptor],
) -> BumpAllocator<'balloc> {
    debug!("Creating virtual addressor for kernel (starting at 0x0, identity-mapped).");

    let virtual_addressor = unsafe {
        VIRTUAL_ADDESSOR.replace(VirtualAddressor::new(VirtAddr::zero()));
        VIRTUAL_ADDESSOR.get_mut()
    };

    debug!("Identity mapping all reserved memory.");
    for frame in memory_map
        .iter()
        .filter(|descriptor| gsai::memory::is_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| {
            Frame::range_count(PhysAddr::new(descriptor.phys_start), descriptor.page_count)
        })
    {
        virtual_addressor.identity_map(&frame);
    }

    virtual_addressor
        .modify_mapped_addr(global_memory(|allocator| allocator.physical_mapping_addr()));
    virtual_addressor.swap_into();

    BumpAllocator::new(virtual_addressor)
}
