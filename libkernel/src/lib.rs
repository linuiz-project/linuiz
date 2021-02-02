#![no_std]
#![feature(
    asm,
    const_fn,
    once_cell,
    const_panic,
    const_mut_refs,
    abi_x86_interrupt,
    panic_info_message,
    alloc_error_handler,
    const_raw_ptr_to_usize_cast
)]

#[macro_use]
extern crate log;
extern crate alloc;

mod bitarray;

use crate::memory::{paging::VirtualAddressor, UEFIMemoryDescriptor};
use core::{alloc::Layout, panic::PanicInfo};
use efi_boot::BootInfo;

pub mod instructions;
pub mod io;
pub mod memory;
pub mod registers;
pub mod structures;
pub use bitarray::*;
pub use x86_64::{PhysAddr, VirtAddr};

pub const SYSTEM_SLICE_SIZE: usize = 0x10000000000;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap(),
        info.message().unwrap()
    );

    loop {}
}

#[alloc_error_handler]
fn alloc_error(error: Layout) -> ! {
    error!("{:#?}", error);

    loop {}
}

pub fn init(boot_info: &BootInfo<UEFIMemoryDescriptor>) {
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    let frame_map_frames = unsafe { crate::memory::init_global_memory(boot_info.memory_map()) };
    let mut global_addressor = init_global_addressor(boot_info.memory_map());
    frame_map_frames.for_each(|frame| global_addressor.identity_map(&frame));

    debug!("Setting global addressor (`alloc::*` will be usable after this point).");
    unsafe { crate::memory::set_global_addressor(global_addressor) };
}

unsafe fn init_cpu_state() {
    crate::instructions::init_segment_registers(0x0);
    info!("Zeroed segment registers.");
}

fn init_structures() {
    crate::structures::gdt::init();
    info!("Successfully initialized GDT.");
    crate::structures::idt::init();
    info!("Successfully initialized IDT.");
    crate::structures::pic::init();
    info!("Successfully initialized PIC.");
}

fn init_global_addressor<'balloc>(
    memory_map: &[crate::memory::UEFIMemoryDescriptor],
) -> VirtualAddressor {
    use crate::memory::{Frame, Page};

    let mut global_addressor = unsafe { VirtualAddressor::new(Page::null()) };

    debug!("Identity mapping all reserved memory blocks.");
    for frame in memory_map
        .iter()
        .filter(|descriptor| crate::memory::is_uefi_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| {
            Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
        })
    {
        global_addressor.identity_map(&frame);
    }

    debug!("Mapping provided bootloader stack as kernel stack.");
    const STACK_ADDRESS: usize = SYSTEM_SLICE_SIZE * 0xB;

    // We have to allocate a new stack (and copy the old one).
    //
    // To make things fun, there's no pre-defined 'this is a stack'
    //  descriptor. So, as a work-around, we read `rsp`, and find the
    //  descriptor which contains it. I believe this is a flawless solution
    //  that has no possibility of backfiring.
    let rsp_addr = crate::registers::stack::RSP::read();
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
        unsafe { crate::memory::global_lock(&frame) };
    }

    // Since we're using physical offset mapping for our page table modification strategy, the memory needs to be offset identity mapped.
    let phys_mapping_addr = VirtAddr::new((0x1000000000000 - crate::memory::global_total()) as u64);
    debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
    global_addressor.modify_mapped_page(Page::from_addr(phys_mapping_addr));

    unsafe {
        // Swap the PML4 into CR3
        debug!("Writing kernel addressor's PML4 to the CR3 register.");
        global_addressor.swap_into();
        // Adjust `rsp` so it points to our `STACK_ADDRESS` mapping,
        //  plus its current offset from base.
        debug!("Modifying RSP to point to new stack mapping.");
        crate::registers::stack::RSP::add(stack_offset);
    }

    // Now unmap the temporary identity mappings, and our
    //  virtual addressoris fully initialized.
    for frame in stack_descriptor.frame_iter() {
        global_addressor.unmap(&Page::from_index(frame.index()));
    }

    global_addressor
}
