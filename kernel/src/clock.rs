#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

pub struct AtomicClock(AtomicU64);

impl AtomicClock {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn tick(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get_ticks(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

pub mod global {
    static GLOBAL_CLOCK: SyncOnceCell<super::AtomicClock> = SyncOnceCell::new();

    pub fn init() {
        if let Ok(()) = GLOBAL_CLOCK.set(super::AtomicClock::new()) {
            lib::instructions::interrupts::without_interrupts(|| {
                use lib::structures::pic8259;

                pic8259::enable(pic8259::InterruptLines::TIMER);
                pic8259::pit::set_timer_freq(1000, pic8259::pit::OperatingMode::RateGenerator);
                lib::structures::idt::set_handler_fn(
                    crate::local_state::InterruptVector::GlobalTimer as u8,
                    tick_handler,
                );

                debug!("Global clock configured at 1000Hz.");
            })
        } else {
            panic!("Global clock already configured.");
        }
    }

    use lib::{cell::SyncOnceCell, structures::idt::InterruptStackFrame};
    extern "x86-interrupt" fn tick_handler(_: InterruptStackFrame) {
        unsafe {
            GLOBAL_CLOCK
                .get()
                // This handler will only be called after GLOBAL_CLOCK is already initialized.
                .unwrap_unchecked()
        }
        .tick();

        use lib::structures::pic8259;
        pic8259::end_of_interrupt(pic8259::InterruptOffset::Timer);
    }

    pub fn get_ticks() -> Option<u64> {
        GLOBAL_CLOCK
            .get()
            .map(|global_clock| global_clock.get_ticks())
    }

    pub fn busy_wait_msec(milliseconds: u64) {
        let target_ticks = get_ticks().unwrap() + milliseconds;
        while get_ticks().unwrap() <= target_ticks {}
    }
}

pub mod local {
    #[inline(always)]
    pub fn get_ticks() -> u64 {
        crate::local_state::clock().get_ticks()
    }

    #[inline(always)]
    pub fn sleep_msec(milliseconds: u64) {
        let target_ticks = get_ticks() + milliseconds;

        while get_ticks() <= target_ticks {
            lib::instructions::hlt();
        }
    }

    pub struct Timer {
        ticks: u64,
    }

    impl Timer {
        pub fn new(ticks: u64) -> Self {
            Self { ticks }
        }

        pub fn wait_new(ticks: u64) {
            let timer = Self::new(ticks);
            timer.wait();
        }

        pub fn wait(&self) {
            let end_tick = self.ticks + get_ticks();
            while get_ticks() < end_tick {}
        }
    }

    pub struct Stopwatch {
        start_tick: Option<u64>,
        stop_tick: Option<u64>,
    }

    impl Stopwatch {
        pub const fn new() -> Self {
            Self {
                start_tick: None,
                stop_tick: None,
            }
        }

        pub fn start_new() -> Self {
            Self {
                start_tick: Some(get_ticks()),
                stop_tick: None,
            }
        }

        pub fn start(&mut self) {
            self.stop_tick = None;
            self.start_tick = Some(get_ticks());
        }

        pub fn stop(&mut self) {
            match self.start_tick {
                Some(_) => self.stop_tick = Some(get_ticks()),
                None => panic!("stopwatch not currently running"),
            }
        }

        pub fn elapsed_ticks(&self) -> u64 {
            let start_tick = self.start_tick.expect("no start tick");
            let stop_tick = self.stop_tick.expect("no stop tick");

            stop_tick - start_tick
        }
    }
}
