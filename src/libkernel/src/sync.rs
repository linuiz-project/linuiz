use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SuccessSource(Arc<AtomicBool>, Arc<AtomicBool>);

impl SuccessSource {
    pub fn new() -> (Self, SuccessToken) {
        let complete = Arc::new(AtomicBool::new(false));
        let success = Arc::new(AtomicBool::new(false));

        let complete_token = Arc::clone(&complete);
        let success_token = Arc::clone(&success);

        (Self(complete, success), SuccessToken(complete_token, success_token))
    }

    pub fn new_valued<T>(value: T) -> (Self, ValuedSuccessToken<T>) {
        let complete = Arc::new(AtomicBool::new(false));
        let success = Arc::new(AtomicBool::new(false));

        let complete_token = Arc::clone(&complete);
        let success_token = Arc::clone(&success);

        (Self(complete, success), ValuedSuccessToken(complete_token, success_token, value))
    }

    pub fn complete(self, success: bool) {
        self.1.store(success, Ordering::Release);
        self.0.store(true, Ordering::Release);
    }
}

pub struct SuccessToken(Arc<AtomicBool>, Arc<AtomicBool>);

impl SuccessToken {
    pub fn busy_wait(self) -> bool {
        while !self.0.load(Ordering::Acquire) {}

        self.1.load(Ordering::Acquire)
    }
}

pub struct ValuedSuccessToken<T>(Arc<AtomicBool>, Arc<AtomicBool>, T);

impl<T> ValuedSuccessToken<T> {
    pub fn busy_wait(self) -> Result<T, ()> {
        while !self.0.load(Ordering::Acquire) {}

        if self.1.load(Ordering::Acquire) {
            Ok(self.2)
        } else {
            Err(())
        }
    }
}
