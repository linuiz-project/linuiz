use core::sync::atomic::AtomicUsize;

static TICKS: AtomicUsize = AtomicUsize::new(0);
static RESOLUTION: AtomicUsize = AtomicUsize::new(8);

fn increment() -> usize {
    TICKS.fetch_add(1, core::sync::atomic::Ordering::AcqRel)
}

fn resolution_mask() -> usize {
    RESOLUTION.load(core::sync::atomic::Ordering::Acquire) - 1
}

pub fn tick_handler() {
    let old_tick = increment();
    if (old_tick & resolution_mask()) == 0 {
        timer_lapse();
    }
}

pub fn timer_lapse() {}
