#![allow(dead_code)]

pub extern "x86-interrupt" fn apic_tick_handler(_: libstd::structures::idt::InterruptStackFrame) {
    // TODO configure APIC timer should be done OUTSIDE the APIC, most likely. It will be
    //      kernel-dependent, and so shouldn't be in libstd.

    unsafe {
        use libstd::registers::MSR;
        MSR::IA32_GS_BASE.write(MSR::IA32_GS_BASE.read() + 1);
    }

    libstd::lpu::try_get().unwrap().apic().end_of_interrupt();
}

#[inline(always)]
pub fn get_ticks() -> u64 {
    unsafe { libstd::registers::MSR::IA32_GS_BASE.read_unchecked() }
}

#[inline(always)]
pub fn sleep_msec(milliseconds: u64) {
    tick_wait(get_ticks() + milliseconds);
}

#[inline(always)]
pub fn sleep_sec(seconds: u64) {
    tick_wait(get_ticks() + (seconds * 1000));
}

#[inline(always)]
fn tick_wait(target_ticks: u64) {
    while get_ticks() < target_ticks {
        libstd::instructions::hlt();
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
