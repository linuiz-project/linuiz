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

pub mod global {
    use core::fmt::Write;

    static GLOBAL_CLOCK: super::AtomicClock = super::AtomicClock::new();

    pub fn start() {
        libkernel::instructions::interrupts::without_interrupts(|| {
            use libkernel::structures::pic8259;

            pic8259::enable(pic8259::InterruptLines::TIMER);
            pic8259::pit::set_timer_freq(1000, pic8259::pit::OperatingMode::RateGenerator);

            unsafe {
                crate::tables::idt::set_handler_fn(
                    crate::local_state::InterruptVector::GlobalTimer as u8,
                    tick_handler,
                )
            };
        });
    }

    fn tick_handler(
        _: &mut crate::tables::idt::InterruptStackFrame,
        _: *mut crate::scheduling::ThreadRegisters,
    ) {
        unsafe {
            crate::CON_OUT
                .write_fmt(format_args!(" {} ", GLOBAL_CLOCK.tick()))
                .unwrap()
        };

        use libkernel::structures::pic8259;
        pic8259::end_of_interrupt(pic8259::InterruptOffset::Timer);
    }

    #[inline]
    pub fn get_ticks() -> u64 {
        GLOBAL_CLOCK.get_ticks()
    }

    pub fn busy_wait_msec(milliseconds: u64) {
        let target_ticks = get_ticks() + milliseconds;
        while get_ticks() <= target_ticks {
            libkernel::instructions::pause();
        }
    }
}

pub mod local {
    #[inline(always)]
    pub fn get_ticks() -> u64 {
        // TODO this is fucked in userland ?? swapgs sucks
        unsafe { crate::local_state::clock().get_ticks() }
    }

    #[inline(always)]
    pub fn sleep_msec(milliseconds: u64) {
        let target_ticks = get_ticks() + milliseconds;

        while get_ticks() <= target_ticks {
            libkernel::instructions::hlt();
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
        #[inline]
        pub const fn new() -> Self {
            Self {
                start_tick: None,
                stop_tick: None,
            }
        }

        #[inline]
        pub fn start_new() -> Self {
            Self {
                start_tick: Some(get_ticks()),
                stop_tick: None,
            }
        }

        #[inline]
        pub fn start(&mut self) {
            self.stop_tick = None;
            self.start_tick = Some(get_ticks());
        }

        #[inline]
        pub fn stop(&mut self) {
            match self.start_tick {
                Some(_) => self.stop_tick = Some(get_ticks()),
                None => panic!("stopwatch not currently running"),
            }
        }

        #[inline]
        pub fn restart(&mut self) {
            self.start_tick = Some(get_ticks());
            self.stop_tick = None;
        }

        #[inline]
        pub fn elapsed_ticks(&self) -> u64 {
            self.start_tick.unwrap_or(0) - self.stop_tick.unwrap_or(0)
        }
    }
}
