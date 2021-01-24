#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate log;
extern crate alloc;

use core::ffi::c_void;
use efi_boot::BootInfo;
use gsai::memory::{
    allocators::{global_memory, init_global_allocator},
    paging::VirtualAddressorCell,
    Frame, UEFIMemoryDescriptor,
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
    unsafe { gsai::memory::allocators::init_global_memory(boot_info.memory_map()) };
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

    //x86_64::instructions::interrupts::enable();
    warn!("Interrupts are now enabled!");
}

fn init_virtual_addressor<'balloc>(memory_map: &[gsai::memory::UEFIMemoryDescriptor]) {
    use gsai::memory::Page;
    use x86_64::VirtAddr;

    debug!("Creating virtual addressor for kernel (starting at 0x0, identity-mapped).");
    KERNEL_ADDRESSOR.init(Page::null());

    for frame in memory_map
        .iter()
        .filter(|descriptor| gsai::memory::is_reserved_memory_type(descriptor.ty))
        .flat_map(|descriptor| Frame::range_count(descriptor.phys_start, descriptor.page_count))
    {
        KERNEL_ADDRESSOR.identity_map(&frame);
    }

    debug!("Validating all kernel sections are mapped for addressor (using static linker labels).");
    validate_section_mappings(&KERNEL_ADDRESSOR);

    // let physical_mapping_addr = global_memory(|allocator| allocator.physical_mapping_addr());
    // KERNEL_ADDRESSOR.modify_mapped_page(Page::from_addr(physical_mapping_addr));

    let virt_addr = VirtAddr::new(0xff06138);
    if !KERNEL_ADDRESSOR.is_mapped(virt_addr) {
        for descriptor in memory_map.iter() {
            if descriptor.range().contains(&virt_addr.as_u64()) {
                info!("{:?} CONTAINED BY:\n{:#?}", virt_addr, descriptor);
            }
        }
    }

    unsafe { KERNEL_ADDRESSOR.swap_into() };

    init_global_allocator(&KERNEL_ADDRESSOR);
}

pub fn validate_section_mappings(virtual_addressor: &VirtualAddressorCell) {
    fn validate_section(virtual_addressor: &VirtualAddressorCell, section: core::ops::Range<u64>) {
        for virt_addr in section
            .step_by(0x1000)
            .map(|addr| x86_64::VirtAddr::new(addr))
        {
            if !virtual_addressor.is_mapped(virt_addr) {
                panic!(
                    "failed to validate section: addr {:?} not mapped",
                    virt_addr
                );
            }
        }
    }

    let text_section = _text();
    debug!("Validating .text section ({:?})...", text_section);
    validate_section(virtual_addressor, text_section);

    let rodata_section = _rodata();
    debug!("Validating .rodata section ({:?})...", rodata_section);
    validate_section(virtual_addressor, rodata_section);

    let data_section = _data();
    debug!("Validating .data section ({:?})...", data_section);
    validate_section(virtual_addressor, data_section);

    let bss_section = _bss();
    debug!("Validating .bss section ({:?})...", bss_section);
    validate_section(virtual_addressor, bss_section);

    debug!("Validated all sections.");
}

pub fn _text() -> core::ops::Range<u64> {
    unsafe { ((&_text_start) as *const c_void as u64)..((&_text_end) as *const c_void as u64) }
}

pub fn _rodata() -> core::ops::Range<u64> {
    unsafe { ((&_rodata_start) as *const c_void as u64)..((&_rodata_end) as *const c_void as u64) }
}

pub fn _data() -> core::ops::Range<u64> {
    unsafe { ((&_data_start) as *const c_void as u64)..((&_data_end) as *const c_void as u64) }
}

pub fn _bss() -> core::ops::Range<u64> {
    unsafe { ((&_bss_start) as *const c_void as u64)..((&_bss_end) as *const c_void as u64) }
}
