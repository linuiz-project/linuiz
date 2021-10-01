#![allow(dead_code)]

use core::sync::atomic::{AtomicUsize, Ordering};

static TICKS: AtomicUsize = AtomicUsize::new(0);

// Frequency of timer, or ticks per second.
pub const FREQUENCY: usize = 1000;

pub extern "x86-interrupt" fn tick_handler(_: libkernel::structures::idt::InterruptStackFrame) {
    TICKS.fetch_add(1, Ordering::AcqRel);
    crate::pic8259::end_of_interrupt(crate::pic8259::InterruptOffset::Timer);
}

pub extern "x86-interrupt" fn apic_tick_handler(
    _: libkernel::structures::idt::InterruptStackFrame,
) {
    TICKS.fetch_add(1, Ordering::AcqRel);
    libkernel::structures::apic::local_apic_mut()
        .unwrap()
        .end_of_interrupt();
}

#[inline(always)]
pub fn get_ticks() -> usize {
    TICKS.load(Ordering::Acquire)
}

#[inline(always)]
pub fn get_ticks_unordered() -> usize {
    TICKS.load(Ordering::Relaxed)
}

#[inline(always)]
pub fn sleep_msec(milliseconds: usize) {
    tick_wait(get_ticks() + milliseconds);
}

#[inline(always)]
pub fn sleep_sec(seconds: usize) {
    tick_wait(get_ticks() + (seconds * FREQUENCY));
}

#[inline(always)]
fn tick_wait(target_ticks: usize) {
    while get_ticks() < target_ticks {
        libkernel::instructions::hlt();
    }
}

pub struct Timer {
    ticks: usize,
}

impl Timer {
    pub fn new(ticks: usize) -> Self {
        Self { ticks }
    }

    pub fn wait_new(ticks: usize) {
        let timer = Self::new(ticks);
        timer.wait();
    }

    pub fn wait(&self) {
        let end_tick = self.ticks + get_ticks();
        while get_ticks() < end_tick {}
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
