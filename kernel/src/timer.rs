use core::sync::atomic::AtomicUsize;

static TICKS: AtomicUsize = AtomicUsize::new(0);
static INTERVAL: AtomicUsize = AtomicUsize::new(1);

fn increment() -> usize {
    TICKS.fetch_add(1, core::sync::atomic::Ordering::AcqRel)
}

fn interval_mask() -> usize {
    INTERVAL.load(core::sync::atomic::Ordering::Acquire) - 1
}

pub fn set_interval(interval: usize) {
    if !interval.is_power_of_two() {
        panic!("timer interval must be a power of 2");
    } else {
        INTERVAL.store(interval, core::sync::atomic::Ordering::Release);
    }
}

pub fn tick_handler() {
    let old_tick = increment();
    if (old_tick & interval_mask()) == 0 {
        timer_lapse();
    }
}

pub fn timer_lapse() {
    info!("{}", TICKS.load(core::sync::atomic::Ordering::Acquire));
}
