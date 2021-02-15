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
    instructions::{cpuid_features, CPUFeatures},
    memory::UEFIMemoryDescriptor,
    registers::MSR,
    structures, BootInfo, ConfigTableEntry,
};
use structures::{apic::APIC, pic::pic8259::PIC_8259_HZ};

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

    init_apic_timer();

    loop {}

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    unsafe { libkernel::memory::init_global_allocator(boot_info.memory_map()) };

    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_apic_timer() {
    use crate::structures::apic::{
        APICDeliveryMode, APICRegister, APICTimerDivisor, APICTimerMode, InvariantAPICRegister,
    };
    use timer::Timer;

    let mut apic = APIC::from_msr();

    // Initialize to known state
    apic[InvariantAPICRegister::DFR] = 0xFFFFFFFF;
    let mut ldr = apic[InvariantAPICRegister::LDR];
    ldr &= 0xFFFFFF;
    ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
    apic[InvariantAPICRegister::LDR] = ldr;
    apic.performance().set_delivery_mode(APICDeliveryMode::NMI);
    apic.lint0().set_masked(true);
    apic.lint1().set_masked(true);
    apic[InvariantAPICRegister::TaskPriority] = 0;
    apic.timer().set_masked(true);

    // Enable APIC
    unsafe {
        MSR::IA32_APIC_BASE.write(MSR::IA32_APIC_BASE.read() | (1 << 10));
    }

    // Map spurious to dummy ISR
    apic.set_spurious((39 + APIC::SW_ENABLE) as u8);
    // Map APIC timer to an interrupt, and by that enable it in one-shot mode (APICTimerMode = 0x0)
    {
        let mut timer = apic.timer();
        timer.set_vector(32);
        timer.set_mode(APICTimerMode::OneShot);
        timer.set_masked(false);
    }
    // Tell the APIC timer to use a divider of 16
    apic[InvariantAPICRegister::TimerDivisor] = 0x3;

    // Construct our timer to wait 10ms
    let waiter = Timer::new(10 * (PIC_8259_HZ / 1000));

    // Set APIC init counter to -1
    apic[InvariantAPICRegister::TimerInitialCount] = 0xFFFFFFFF;

    // Wait 10 ms
    waiter.wait();

    // mask timer and then configure
    {
        let mut timer = apic.timer();
        timer.set_masked(true);
        timer.set_mode(APICTimerMode::Periodic);
    }

    let ticks_in_10ms = 0xFFFFFFFF - (apic[InvariantAPICRegister::TimeCurrentCount] as u32);
    apic[InvariantAPICRegister::TimerInitialCount] = ticks_in_10ms as u128;
    apic[InvariantAPICRegister::TimerDivisor] = APICTimerDivisor::Div16 as u128;

    unsafe { libkernel::structures::pic::disable() };

    loop {
        info!("{:?}", crate::timer::get_ticks());
    }
}
