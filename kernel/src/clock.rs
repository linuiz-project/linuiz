#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

pub struct AtomicClock(AtomicU64);

impl AtomicClock {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    #[inline]
    pub fn tick(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    pub fn get_ticks(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

use liblz::structures::pic8259;

static GLOBAL_CLOCK: AtomicClock = AtomicClock::new();

pub fn configure_and_enable() {
    liblz::instructions::interrupts::without_interrupts(|| {
        pic8259::pit::set_timer_freq(1000, pic8259::pit::OperatingMode::RateGenerator);
        pic8259::enable(pic8259::InterruptLines::TIMER);

        unsafe {
            crate::interrupts::set_handler_fn(
                crate::interrupts::Vector::GlobalTimer,
                global_timer_handler,
            )
        };
    });
}

fn global_timer_handler(
    _: &mut x86_64::structures::idt::InterruptStackFrame,
    _: *mut crate::scheduling::ThreadRegisters,
) {
    GLOBAL_CLOCK.tick();

    pic8259::end_of_interrupt(pic8259::InterruptOffset::Timer);
}

#[inline]
pub fn get_ticks() -> u64 {
    GLOBAL_CLOCK.get_ticks()
}

/// Waits for the specified number of milliseconds.
pub fn busy_wait_msec(milliseconds: u64) {
    let target_ticks = get_ticks() + milliseconds;
    while get_ticks() <= target_ticks {
        liblz::instructions::pause();
    }
}