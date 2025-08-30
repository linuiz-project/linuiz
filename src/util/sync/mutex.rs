use crate::interrupts::uninterruptable;
use core::{
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{Atomic, Ordering},
};

/// A [spin lock](https://en.m.wikipedia.org/wiki/Spinlock) providing mutually exclusive access to data.
pub struct Mutex<T> {
    lock: Atomic<bool>,
    data: UnsafeCell<T>,
}

// Safety: Same unsafe impls as `std::sync::Mutex`.
unsafe impl<T: Send> Send for Mutex<T> {}
// Safety: Same unsafe impls as `std::sync::Mutex`.
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] wrapping the supplied data.
    ///
    /// # Example
    ///
    /// ```
    /// use crate::util::sync::Mutex;
    ///
    /// static MUTEX: Mutex<()> = Mutex::<_>::new(());
    ///
    /// fn demo() {
    ///     let lock = MUTEX.lock();
    ///     // do something with lock
    ///     drop(lock);
    /// }
    /// ```
    pub const fn new(data: T) -> Self {
        Self {
            lock: Atomic::<bool>::new(false),
            data: UnsafeCell::new(data),
        }
    }

    fn data_ptr(&self) -> NonNull<T> {
        let ptr = self.data.get();
        // Safety: `ptr` comes from an occupied `UnsafeCell<T>`.
        unsafe { NonNull::new_unchecked(ptr) }
    }

    /// Returns `true` if the lock is currently held.
    ///
    /// # Safety
    ///
    /// This function provides no synchronization guarantees and so its result
    /// should be considered 'out of date' the instant it is called. Do not use
    /// it for synchronization purposes. However, it may be useful as a
    /// heuristic.
    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }

    pub fn with_lock<'a, U>(&'a self, func: impl FnOnce(&'a mut T) -> U) -> U {
        uninterruptable(|| {
            // Can fail to lock even if the spinlock is not locked. May be more efficient
            // than `try_lock` when called in a loop.
            loop {
                // There's some debate about whether `Ordering::Acquire` is sufficient here. In
                // one interpretation of the the C++ standard, a lock operation like this could
                // be re-ordered with a prior unlock of a different mutex. In other words
                // this...
                //
                //     let a_guard = A.lock();
                //     // do stuff...
                //     drop(a_guard);
                //     let b_guard = B.lock();
                //     // do more stuff...
                //     drop(b_guard);
                //
                //  ... could be reordered by the compiler into this...
                //
                //     let a_guard = A.lock();
                //     let b_guard = B.lock();
                //     // do stuff...
                //     // do more stuff...
                //     drop(a_guard);
                //     drop(b_guard);
                //
                //  ...because both the store-release in `drop(a_guard)` and the load-acquire in
                // `B.lock()`  allow this code movement. (Using `Ordering::AcqRel` here instead
                // would forbid this, because nothing can move down across a store-release, but
                // it would also prevent valid optimizations). The worry is that this could lead
                // to deadlocks in arguably correct programs, for example one thread locking
                // A-then-B while another thread locks B-then-A, even though astraight-line
                // reading of the code says that can't happen.
                //
                // However, there's another interpretation of the standard that says this
                // reordering is illegal. The idea is that even though moving a
                // (non-sequentially-consistent) store-release down across a load-acquire is ok,
                // moving it down across an *unbounded loop* violates the requirement that
                // atomic stores should be visible to other threads in a "finite period of
                // time": https://eel.is/c++draft/basic#intro.progress-18. This seems to be how
                // compilers currently behave, in  any case.
                //
                // See also:
                //  - https://preshing.com/20170612/can-reordering-of-release-acquire-operations-introduce-deadlock
                //  - https://x.com/tvaneerd/status/1258426442649657346
                //  - https://youtu.be/A8eCGOqgvH4?t=2551
                if self
                    .lock
                    .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    // Using a block here ensures the exlusive borrow of `data` does not escape.
                    let func_value = {
                        // Safety: `self.data` is atomically checked to allow exclusive access.
                        let data = unsafe { self.data.as_mut_unchecked() };

                        func(data)
                    };

                    // Release the exclusive borrow.
                    self.lock.store(false, Ordering::Release);

                    return func_value;
                }

                while self.is_locked() {
                    core::hint::spin_loop();
                }
            }
        })
    }
}
