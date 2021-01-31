use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use spin::RwLock;

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
    let ticks = TICKS.load(core::sync::atomic::Ordering::Acquire);
    for callback in LAPSE_CALLBACKS.read().iter() {
        (callback)(ticks);
    }
}

static LAPSE_CALLBACKS: RwLock<Vec<fn(usize)>> = RwLock::new(Vec::new());

pub fn add_callback(callback: fn(usize)) {
    debug!("Adding timer lapse callback: fn(usize) @{:?}", callback);
    LAPSE_CALLBACKS.write().push(callback);
}
