use crate::interrupts::uninterruptable;
use spin::Once;

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
use x86_64::*;

static SYSTEM_CLOCK: Once<SystemClock> = Once::new();

struct SystemClock(ArchClock);

impl SystemClock {
    pub fn init() {
        SYSTEM_CLOCK.call_once(|| uninterruptable(|| Self(ArchClock::configure())));
    }

    fn get_static() -> &'static Self {
        SYSTEM_CLOCK.wait()
    }
}
