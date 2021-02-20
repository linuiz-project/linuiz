#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod drivers;
mod logging;
mod pic8259;
mod timer;

use core::ffi::c_void;
use libkernel::{memory::UEFIMemoryDescriptor, BootInfo, ConfigTableEntry};

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
extern "efiapi" fn kernel_main(boot_info: BootInfo<UEFIMemoryDescriptor, ConfigTableEntry>) -> ! {
    match crate::logging::init_logger(crate::logging::LoggingModes::SERIAL, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
            debug!("Minimum logging level configured as: {:?}", get_log_level());
        }
        Err(error) => panic!("{}", error),
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();

    debug!(
        "Detected CPU features: {:?}",
        libkernel::instructions::cpu_features()
    );

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");
    crate::pic8259::enable();
    info!("Successfully initialized PIC.");
    info!("Configuring PIT frequency to 1000Hz.");
    crate::pic8259::set_timer_freq(crate::timer::TIMER_FREQUENCY as u32);

    // `boot_info` will not be usable after initalizing the global allocator,
    //  due to the stack being moved in virtual memory.
    let framebuffer_pointer = boot_info.framebuffer_pointer().unwrap().clone();
    info!("Initializing global memory (frame allocator, global allocator, et al).");
    unsafe { libkernel::memory::init_global_allocator(boot_info.memory_map()) };

    debug!("Enabling interrupts.");
    libkernel::structures::idt::set_interrupt_handler(32, crate::timer::tick_handler);
    libkernel::instructions::interrupts::enable();

    libkernel::structures::apic::local::load();
    let lapic = libkernel::structures::apic::local::local_apic_mut().unwrap();

    unsafe {
        debug!("Resetting and enabling local APIC (it may have already been enabled).");
        lapic.reset();
        lapic.enable();
        let timer = timer::Timer::new(crate::timer::TIMER_FREQUENCY / 1000);
        lapic.configure_spurious(u8::MAX, true);
        lapic.configure_timer(48, || timer.wait())
    }

    debug!("Disabling 8259 emulated PIC.");
    unsafe { crate::pic8259::disable() };
    debug!("Updating IDT timer interrupt entry to local APIC-enabled function.");
    libkernel::structures::idt::set_interrupt_handler(48, timer::apic_timer_handler);
    debug!("Unmasking local APIC timer interrupt (it will fire now!).");
    lapic.timer().set_masked(false);

    info!("Core-local APIC configured and enabled (8259 PIC disabled).");

    info!("Initializing framebuffer driver.");
    let mut framebuffer_driver = drivers::graphics::framebuffer::FramebufferDriver::init(
        libkernel::PhysAddr::new(framebuffer_pointer.pointer as u64),
        framebuffer_pointer.size,
    );

    let mut vecc = alloc::vec![0usize; 50];
    for (idx, a) in vecc.iter_mut().enumerate() {
        *a = idx;
    }
    info!("{:?}", vecc);

    info!("Testing framebuffer driver.");
    for x in 0..300 {
        for y in 0..300 {
            framebuffer_driver
                .write_pixel((x, y), drivers::graphics::color::Color8i::new(156, 10, 100));
        }
    }

    framebuffer_driver.flush_pixels();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}
