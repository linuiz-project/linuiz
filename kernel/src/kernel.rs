#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt)]

#[macro_use]
extern crate log;
extern crate alloc;

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

    init_apic_timer();

    // info!("Initializing framebuffer driver.");
    // let framebuffer_driver = drivers::graphics::framebuffer::FramebufferDriver::init(
    //     libkernel::PhysAddr::new(framebuffer_pointer.pointer as u64),
    //     framebuffer_pointer.size,
    // );
    //
    // info!("Testing framebuffer driver.");
    // for x in 0..300 {
    //     for y in 0..300 {
    //         framebuffer_driver
    //             .write_pixel((x, y), drivers::graphics::color::Color8i::new(156, 10, 100));
    //     }
    // }

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_apic_timer() {
    use libkernel::structures::apic::local::{
        local_apic_mut, LocalAPICRegister, LocalAPICTimerDivisor, LocalAPICTimerMode,
    };

    libkernel::structures::apic::local::load();
    libkernel::structures::apic::local::reset();
    loop {}
    local_apic_mut(|lapic_option| {
        let lapic = match lapic_option {
            Some(lapic) => lapic,
            None => panic!("local APIC not loaded"),
        };

        debug!("Enabling APIC (it may have already been enabled).");
        // unsafe { lapic.enable() };

        debug!("Setting spurious interrupt to dummy ISR.");
        lapic.configure_spurious(255, true);
        debug!("Configuring APIC timer interrupt.");
        lapic.timer().set_vector(192);
        lapic.timer().set_mode(LocalAPICTimerMode::OneShot);
        lapic.timer().set_masked(false);

        // Tell the APIC timer to use a divisor of 16
        lapic[LocalAPICRegister::TimerDivisor] = LocalAPICTimerDivisor::Div16 as u32;
        debug!("Determining APIC timer frequency using PIT windowing.");
        lapic[LocalAPICRegister::TimerInitialCount] = 0xFFFFFFFF;
    });

    timer::Timer::wait_new(crate::timer::TIMER_FREQUENCY / 1000);
    loop {}
    local_apic_mut(|lapic_option| {
        let lapic = match lapic_option {
            Some(lapic) => lapic,
            None => panic!("local APIC not loaded"),
        };

        // mask timer and then configure
        lapic.timer().set_masked(true);
        let window_ticks = 0xFFFFFFFF - lapic[LocalAPICRegister::TimeCurrentCount];
        debug!(
            "Determined a total of {} APIC timer ticks per {}ms.",
            window_ticks,
            crate::timer::TIMER_FREQUENCY / 1000
        );
        lapic[LocalAPICRegister::TimerInitialCount] = window_ticks as u32;
        lapic[LocalAPICRegister::TimerDivisor] = LocalAPICTimerDivisor::Div16 as u32;
        lapic.timer().set_mode(LocalAPICTimerMode::Periodic);
        lapic.timer().set_vector(32);

        debug!("Disabling 8259 emulated PIC.");
        unsafe { crate::pic8259::disable() };
        debug!("Unmasking APIC timer interrupt (it will fire now!).");
        lapic.timer().set_masked(false);
    });
}
