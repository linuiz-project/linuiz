use core::sync::atomic::AtomicUsize;

static TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn tick_handler() {
    TICKS.fetch_add(1, core::sync::atomic::Ordering::Release);
}

pub fn get_ticks() -> usize {
    TICKS.load(core::sync::atomic::Ordering::Acquire)
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
