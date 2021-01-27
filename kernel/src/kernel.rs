#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;
extern crate alloc;

use core::ffi::c_void;
use efi_boot::BootInfo;
use gsai::memory::{
    global_lock, global_total, paging::VirtualAddressorCell, BlockAllocator, Frame, Page,
    UEFIMemoryDescriptor,
};
use x86_64::VirtAddr;

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

static KERNEL_ADDRESSOR: VirtualAddressorCell = VirtualAddressorCell::empty();

#[export_name = "_start"]
extern "win64" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor>) -> ! {
    match gsai::logging::init_logger(gsai::logging::LoggingModes::SERIAL, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
            debug!("Minimum logging level configured as: {:?}", get_log_level());
        }
        Err(error) => panic!("{}", error),
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();
    info!("Configuring CPU state.");
    unsafe { init_cpu_state() };
    info!("Initializing memory structures.");
    init_structures();

    info!("Initializing global memory (physical frame allocator / journal).");
    let used_pages_iter = unsafe { gsai::memory::init_global_memory(boot_info.memory_map()) };
    init_virtual_addressor(boot_info.memory_map());

    for frame in used_pages_iter {
        KERNEL_ADDRESSOR.identity_map(&frame);
    }

    info!("Initializing global allocator (`alloc::*` usable after this point).");
    // init_global_allocator(&KERNEL_ADDRESSOR);

    let balloc = BlockAllocator::new(Page::from_addr(VirtAddr::new(0x7A12000)), &KERNEL_ADDRESSOR);
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
    info!("Interrupts are now enabled.");
}

fn init_virtual_addressor<'balloc>(memory_map: &[gsai::memory::UEFIMemoryDescriptor]) {
    debug!("Creating virtual addressor for kernel (starting at 0x0, identity-mapped).");
    KERNEL_ADDRESSOR.init(Page::null());

    debug!("Identity mapping all reserved memory blocks.");
    for frame in memory_map
        .iter()
        .filter(|descriptor| gsai::memory::is_uefi_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| {
            Frame::range_count(descriptor.phys_start, descriptor.page_count as usize)
        })
    {
        KERNEL_ADDRESSOR.identity_map(&frame);
    }

    debug!("Validating all kernel sections are mapped for addressor (using extern linker labels).");
    validate_program_segment_mappings(&KERNEL_ADDRESSOR);

    debug!("Mapping provided bootloader stack as kernel stack.");
    const STACK_ADDRESS: usize = gsai::SYSTEM_SLICE_SIZE * 0xB;

    // We have to allocate a new stack (and copy the old one).
    //
    // To make things fun, there's no pre-defined 'this is a stack'
    //  descriptor. So, as a work-around, we read `rsp`, and find the
    //  descriptor which contains it. I believe this is a flawless solution
    //  that has no possibility of backfiring.
    let rsp_addr = gsai::registers::stack::RSP::read();
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
        KERNEL_ADDRESSOR.identity_map(&frame);
        KERNEL_ADDRESSOR.map(&base_offset_page.offset(frame.index()), &frame);
        unsafe { global_lock(&frame) };
    }

    // Since we're using physical offset mapping for our page table modification strategy, the memory needs to be offset identity mapped.
    let phys_mapping_addr = VirtAddr::new((0x1000000000000 - global_total()) as u64);
    debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
    KERNEL_ADDRESSOR.modify_mapped_page(Page::from_addr(phys_mapping_addr));

    unsafe {
        // Swap the PML4 into CR3
        debug!("Writing kernel addressor's PML4 to the CR3 register.");
        KERNEL_ADDRESSOR.swap_into();
        // Adjust `rsp` so it points to our `STACK_ADDRESS` mapping,
        //  plus its current offset from base.
        debug!("Modifying RSP to point to new stack mapping.");
        gsai::registers::stack::RSP::add(stack_offset);
    }

    // Now unmap the temporary identity mappings, and our
    //  virtual addressoris fully initialized.
    for frame in stack_descriptor.frame_iter() {
        KERNEL_ADDRESSOR.unmap(&Page::from_index(frame.index()));
    }
}

pub fn validate_program_segment_mappings(virtual_addressor: &VirtualAddressorCell) {
    fn validate_segment_mapping(
        virtual_addressor: &VirtualAddressorCell,
        section: core::ops::Range<u64>,
    ) {
        for page in section
            .step_by(0x1000)
            .map(|addr| gsai::memory::Page::from_addr(x86_64::VirtAddr::new(addr)))
        {
            if !virtual_addressor.is_mapped_to(&page, &Frame::from_index(page.index())) {
                panic!(
                    "failed to validate section: page {:?} not identity mapped",
                    page
                );
            }
        }
    }

    fn _text() -> core::ops::Range<u64> {
        unsafe { ((&_text_start) as *const c_void as u64)..((&_text_end) as *const c_void as u64) }
    }

    fn _rodata() -> core::ops::Range<u64> {
        unsafe {
            ((&_rodata_start) as *const c_void as u64)..((&_rodata_end) as *const c_void as u64)
        }
    }

    fn _data() -> core::ops::Range<u64> {
        unsafe { ((&_data_start) as *const c_void as u64)..((&_data_end) as *const c_void as u64) }
    }

    fn _bss() -> core::ops::Range<u64> {
        unsafe { ((&_bss_start) as *const c_void as u64)..((&_bss_end) as *const c_void as u64) }
    }

    let text_section = _text();
    debug!("Validating .text section ({:?})...", text_section);
    validate_segment_mapping(virtual_addressor, text_section);

    let rodata_section = _rodata();
    debug!("Validating .rodata section ({:?})...", rodata_section);
    validate_segment_mapping(virtual_addressor, rodata_section);

    let data_section = _data();
    debug!("Validating .data section ({:?})...", data_section);
    validate_segment_mapping(virtual_addressor, data_section);

    let bss_section = _bss();
    debug!("Validating .bss section ({:?})...", bss_section);
    validate_segment_mapping(virtual_addressor, bss_section);

    debug!("Validated all sections.");
}
