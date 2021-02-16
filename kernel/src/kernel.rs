#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt)]

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

    debug!(
        "Detected CPU features: {:?}",
        libkernel::instructions::cpuid_features()
    );

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");
    libkernel::structures::pic::enable();
    info!("Successfully initialized PIC.");
    info!("Configuring PIT frequency to 1000Hz.");
    libkernel::structures::pic::set_timer_freq(crate::timer::TIMER_FREQUENCY as u32);

    info!("Initializing global memory (frame allocator, global allocator, et al).");
    unsafe { libkernel::memory::init_global_allocator(boot_info.memory_map()) };

    debug!("Enabling interrupts and temporarily configuring the PIT.");
    libkernel::structures::idt::set_interrupt_handler(
        libkernel::structures::pic::InterruptOffset::Timer,
        crate::timer::tick_handler,
    );
    libkernel::instructions::interrupts::enable();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

// fn init_apic_timer() {
//     use bit_field::BitField;
//     use libkernel::structures::{
//         apic::{APICDeliveryMode, APICTimerDivisor, APICTimerMode, InvariantAPICRegister},
//         pic::InterruptOffset,
//     };
//     use timer::Timer;

//     static mut APIC_PTR: *mut u8 = core::ptr::null_mut();

//     debug!("Mapping APIC table.");
//     unsafe {
//         APIC_PTR =
//             alloc::alloc::alloc(core::alloc::Layout::from_size_align(0x1000, 0x1000).unwrap());

//         let frame = libkernel::memory::translate_page(&libkernel::memory::Page::from_ptr(APIC_PTR))
//             .expect("failed to translate allocated page to physical frame");
//         info!(
//             "Allocated a 4096 byte segment at {:?} for APIC table ({:?}).",
//             APIC_PTR, frame
//         );

//         MSR::IA32_APIC_BASE.write_bits(12..36, frame.addr().as_u64().get_bits(12..));
//     };

//     // Initialize to known state
//     let apic = &mut APIC::from_ptr(unsafe { APIC_PTR });
//     debug!("Initializing APIC (@{:?}) to known state.", apic.ptr());
//     apic[InvariantAPICRegister::DFR] = 0xFFFFFFFF;
//     let mut ldr = apic[InvariantAPICRegister::LDR];
//     ldr &= 0xFFFFFF;
//     ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
//     apic[InvariantAPICRegister::LDR] = ldr;
//     debug!("Masking all LVT interrupts.");
//     apic.timer().set_masked(true);
//     apic.performance().set_delivery_mode(APICDeliveryMode::NMI);
//     apic.lint0().set_masked(true);
//     apic.lint1().set_masked(true);
//     apic[InvariantAPICRegister::TaskPriority] = 0;

//     // Enable APIC
//     debug!("Enabling APIC (it may have already been enabled).");
//     unsafe { MSR::IA32_APIC_BASE.write_bit(11, true) };

//     // Map spurious to dummy ISR
//     debug!("Setting spurious interrupt to dummy ISR.");
//     apic[InvariantAPICRegister::Spurious].set_bits(0..=8, 255 | APIC::SW_ENABLE);

//     // Map APIC timer to an interrupt, and by that enable it in one-shot mode (APICTimerMode = 0x0)
//     debug!("Configuring APIC timer interrupt.");
//     fn dummy_apic_handler() {
//         info!(".");
//         APIC::from_ptr(unsafe { APIC_PTR }).signal_eoi()
//     }
//     libkernel::structures::idt::set_interrupt_handler(
//         InterruptOffset::DummyAPIC,
//         dummy_apic_handler,
//     );
//     apic.timer().set_vector(InterruptOffset::DummyAPIC as u8);
//     apic.timer().set_mode(APICTimerMode::OneShot);
//     apic.timer().set_masked(false);

//     // Tell the APIC timer to use a divider of 16
//     apic[InvariantAPICRegister::TimerDivisor] = APICTimerDivisor::Div16 as u128;

//     let waiter = Timer::new(crate::timer::TIMER_FREQUENCY / 1000);

//     // Set APIC init counter to -1
//     apic[InvariantAPICRegister::TimerInitialCount] = 0xFFFFFFFF;

//     // waiter.wait();

//     let mut been_zero_cnt = 0;
//     loop {
//         let cnt = apic[InvariantAPICRegister::TimeCurrentCount] as u64;

//         if cnt == 0 {
//             been_zero_cnt += 1;
//         }
//         info!("{}", cnt);

//         if been_zero_cnt > 10 {
//             apic.timer().set_masked(true);

//             panic!("BEEN ZERO");
//         }
//     }

//     // mask timer and then configure
//     apic.timer().set_masked(true);

//     let window_ticks = 0xFFFFFFFF - (apic[InvariantAPICRegister::TimeCurrentCount] as u32);
//     debug!(
//         "Determined a total of {} APIC timer ticks per {}ms.",
//         window_ticks,
//         crate::timer::TIMER_FREQUENCY / 1000
//     );
//     apic[InvariantAPICRegister::TimerInitialCount] = window_ticks as u128;
//     apic[InvariantAPICRegister::TimerDivisor] = APICTimerDivisor::Div16 as u128;
//     apic.timer().set_mode(APICTimerMode::Periodic);
//     apic.timer().set_vector(32);

//     debug!("Disabling 8259 emulated PIC.");
//     unsafe { libkernel::structures::pic::disable() };
//     debug!("Unmasking APIC timer interrupt (it will fire now!).");
//     apic.timer().set_masked(false);
// }
