use core::sync::atomic::AtomicUsize;

static TICKS: AtomicUsize = AtomicUsize::new(0);

pub fn tick_handler() {
    TICKS.fetch_add(1, core::sync::atomic::Ordering::Release);
}

pub fn get_ticks() -> usize {
    TICKS.load(core::sync::atomic::Ordering::Acquire)
}
