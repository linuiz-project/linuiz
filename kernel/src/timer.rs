#![allow(dead_code)]

use core::sync::atomic::AtomicUsize;

static TICKS: AtomicUsize = AtomicUsize::new(0);

// Frequency of timer, or ticks per second.
pub const TIMER_FREQUENCY: usize = 1000;

pub extern "x86-interrupt" fn tick_handler(
    _: &mut libkernel::structures::idt::InterruptStackFrame,
) {
    TICKS.fetch_add(1, core::sync::atomic::Ordering::Release);
    crate::pic8259::end_of_interrupt(crate::pic8259::InterruptOffset::Timer);
}

pub extern "x86-interrupt" fn apic_timer_handler(
    _: &mut libkernel::structures::idt::InterruptStackFrame,
) {
    info!(".");
    TICKS.fetch_add(1, core::sync::atomic::Ordering::Release);
    libkernel::structures::apic::local::local_apic_mut()
        .unwrap()
        .end_of_interrupt();
}

pub fn get_ticks() -> usize {
    TICKS.load(core::sync::atomic::Ordering::Acquire)
}

pub fn get_ticks_unordered() -> usize {
    TICKS.load(core::sync::atomic::Ordering::Relaxed)
}

pub fn wait(ticks: usize) {
    Timer::wait_new(ticks);
}

pub struct Timer {
    ticks: usize,
    end_tick: usize,
}

impl Timer {
    pub fn new(ticks: usize) -> Self {
        Self { ticks, end_tick: 0 }
    }

    pub fn wait_new(ticks: usize) {
        let timer = Self::new(ticks);
        timer.wait();
    }

    pub fn wait(&self) {
        let end_tick = self.ticks + get_ticks();
        while get_ticks_unordered() < end_tick {}
    }
}

pub struct Stopwatch {
    start_tick: Option<usize>,
    stop_tick: Option<usize>,
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

    pub fn elapsed_ticks(&self) -> usize {
        let start_tick = self.start_tick.expect("no start tick");
        let stop_tick = self.stop_tick.expect("no stop tick");

        stop_tick - start_tick
    }
}
