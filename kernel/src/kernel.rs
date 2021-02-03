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
    BootInfo, VirtAddr,
};

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
extern "efiapi" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor>) -> ! {
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

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    let frame_map_frames = unsafe { libkernel::memory::init_global_memory(boot_info.memory_map()) };
    let mut global_addressor = init_global_addressor(boot_info.memory_map());
    frame_map_frames.for_each(|frame| global_addressor.identity_map(&frame));
    debug!("Setting global addressor (`alloc::*` will be usable after this point).");
    unsafe { libkernel::memory::set_global_addressor(global_addressor) };

    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_global_addressor<'balloc>(
    memory_map: &[libkernel::memory::UEFIMemoryDescriptor],
) -> VirtualAddressor {
    use libkernel::memory::{Frame, Page};

    let mut global_addressor = unsafe { VirtualAddressor::new(Page::null()) };

    debug!("Identity mapping all reserved memory blocks.");
    for frame in memory_map
        .iter()
        .filter(|descriptor| libkernel::memory::is_uefi_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| {
            Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
        })
    {
        global_addressor.identity_map(&frame);
    }

    debug!("Mapping provided bootloader stack as kernel stack.");
    const STACK_ADDRESS: usize = libkernel::SYSTEM_SLICE_SIZE * 0xB;

    // We have to allocate a new stack (and copy the old one).
    //
    // To make things fun, there's no pre-defined 'this is a stack'
    //  descriptor. So, as a work-around, we read `rsp`, and find the
    //  descriptor which contains it. I believe this is a flawless solution
    //  that has no possibility of backfiring.
    let rsp_addr = libkernel::registers::stack::RSP::read();
    // Still, this feels like I'm cheating on a math test
    let stack_descriptor = memory_map
        .iter()
        .find(|descriptor| descriptor.range().contains(&rsp_addr.as_u64()))
        .expect("failed to find stack memory region");
    debug!("Identified stack descriptor:\n{:#?}", stack_descriptor);
    let stack_offset = (STACK_ADDRESS as u64) - stack_descriptor.phys_start.as_u64();
    // this allows `.offset(frame.index())` to align to our actual base address, STACK_ADDRESS
    let base_offset_page = Page::from_addr(VirtAddr::new(stack_offset));
    for frame in stack_descriptor.frame_iter() {
        // This is a temporary identity mapping, purely
        //  so `rsp` isn't invalid after we swap the PML4.
        global_addressor.identity_map(&frame);
        global_addressor.map(&base_offset_page.offset(frame.index()), &frame);
        unsafe { libkernel::memory::global_lock(&frame) };
    }

    // Since we're using physical offset mapping for our page table modification strategy, the memory needs to be offset identity mapped.
    let phys_mapping_addr =
        VirtAddr::new((0x1000000000000 - libkernel::memory::global_total()) as u64);
    debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
    global_addressor.modify_mapped_page(Page::from_addr(phys_mapping_addr));

    unsafe {
        // Swap the PML4 into CR3
        debug!("Writing kernel addressor's PML4 to the CR3 register.");
        global_addressor.swap_into();
        // Adjust `rsp` so it points to our `STACK_ADDRESS` mapping,
        //  plus its current offset from base.
        debug!("Modifying RSP to point to new stack mapping.");
        libkernel::registers::stack::RSP::add(stack_offset);
    }

    // Now unmap the temporary identity mappings, and our
    //  virtual addressoris fully initialized.
    for frame in stack_descriptor.frame_iter() {
        global_addressor.unmap(&Page::from_index(frame.index()));
    }

    global_addressor
}
