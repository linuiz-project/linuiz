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
    memory::UEFIMemoryDescriptor, registers::MSR, structures::apic::APIC, BootInfo,
    ConfigTableEntry,
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

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");
    libkernel::structures::pic::init();
    info!("Successfully initialized PIC.");

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    unsafe { libkernel::memory::init_global_allocator(boot_info.memory_map()) };

    debug!("Enabling interrupts and temporarily configuring the PIT.");
    libkernel::structures::pit::configure_hz(crate::timer::TIMER_FREQUENCY as u32);
    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    init_apic_timer();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_apic_timer() {
    use libkernel::structures::apic::{
        APICDeliveryMode, APICTimerDivisor, APICTimerMode, InvariantAPICRegister,
    };
    use timer::Timer;

    debug!("Mapping APIC table.");
    let apic = unsafe {
        let apic_ptr =
            alloc::alloc::alloc(core::alloc::Layout::from_size_align(0x1000, 0x1000).unwrap());

        let frame = libkernel::memory::translate_page(&libkernel::memory::Page::from_ptr(apic_ptr))
            .expect("failed to translate allocated page to physical frame");
        info!(
            "Allocated a 4096 byte segment at {:?} for APIC table ({:?}).",
            apic_ptr, frame
        );

        MSR::IA32_APIC_BASE.write(frame.addr().as_u64() | (MSR::IA32_APIC_BASE.read() & 0xFFF));
        &mut APIC::from_ptr(apic_ptr)
    };

    // Initialize to known state
    debug!("Initializing APIC to known state.");
    apic[InvariantAPICRegister::DFR] = 0xFFFFFFFF;
    let mut ldr = apic[InvariantAPICRegister::LDR];
    ldr &= 0xFFFFFF;
    ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
    apic[InvariantAPICRegister::LDR] = ldr;
    apic.timer().set_masked(true);
    apic.performance().set_delivery_mode(APICDeliveryMode::NMI);
    apic.lint0().set_masked(true);
    apic.lint1().set_masked(true);
    apic[InvariantAPICRegister::TaskPriority] = 0;

    // Enable APIC
    debug!("Enabling APIC (interrupts not firing yet).");
    unsafe {
        MSR::IA32_APIC_BASE.write(MSR::IA32_APIC_BASE.read() | (1 << 10));
    }

    // Map spurious to dummy ISR
    debug!("Setting spurious interrupt to dummy ISR.");
    apic.set_spurious((39 + APIC::SW_ENABLE) as u8);

    // Map APIC timer to an interrupt, and by that enable it in one-shot mode (APICTimerMode = 0x0)
    debug!("Configuring APIC timer interrupt.");
    apic.timer().set_vector(32);
    apic.timer().set_mode(APICTimerMode::OneShot);
    apic.timer().set_masked(false);

    // Tell the APIC timer to use a divider of 16
    apic[InvariantAPICRegister::TimerDivisor] = 0x3;

    let waiter = Timer::new(crate::timer::TIMER_FREQUENCY / 1000);

    // Set APIC init counter to -1
    apic[InvariantAPICRegister::TimerInitialCount] = 0xFFFFFFFF;

    waiter.wait();

    // mask timer and then configure
    apic.timer().set_masked(true);
    info!("{}", apic[InvariantAPICRegister::LVT_TIMER]);

    let window_ticks = 0xFFFFFFFF - (apic[InvariantAPICRegister::TimeCurrentCount] as u32);
    debug!(
        "Determined a total of {} APIC timer ticks per {}ms.",
        window_ticks,
        crate::timer::TIMER_FREQUENCY / 1000
    );
    apic[InvariantAPICRegister::TimerInitialCount] = window_ticks as u128;
    apic[InvariantAPICRegister::TimerDivisor] = APICTimerDivisor::Div16 as u128;
    apic.timer().set_mode(APICTimerMode::Periodic);
    apic.timer().set_vector(32);

    debug!("Disabling 8259 emulated PIC.");
    unsafe { libkernel::structures::pic::disable() };
    debug!("Unmasking APIC timer interrupt (it will fire now!).");
    apic.timer().set_masked(false);
}
