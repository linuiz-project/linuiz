use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{Atomic, Ordering},
};
use num_enum::{IntoPrimitive, UnsafeFromPrimitive};

struct Finish<'a> {
    status: &'a AtomicStatus,
}

impl Drop for Finish<'_> {
    fn drop(&mut self) {
        // While using Relaxed here would most likely not be an issue, we use
        // `Ordering::SeqCst` anyway. This is mainly because panics are not meant to be
        // fast at all, but also because if there were to be a compiler bug which
        // reorders accesses within the same thread, where it should not, we want to be
        // sure that the panic really is handled, and does not cause additional
        // problems. `Ordering::SeqCst` will therefore help guarding against such bugs.
        self.status.store(Status::Panicked, Ordering::SeqCst);
    }
}

// SAFETY: This structure has an invariant, namely that the inner atomic u8 must
// *always* have a value for which there exists a valid Status. This means that
// users of this API must only be allowed to load and store `Status`es.
#[repr(transparent)]
pub struct AtomicStatus(Atomic<u8>);

// Four states that a Once can be in, encoded into the lower bits of `status` in
// the Once structure.
#[repr(u8)]
#[derive(IntoPrimitive, UnsafeFromPrimitive, Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Incomplete = 0x00,
    Running = 0x01,
    Complete = 0x02,
    Panicked = 0x03,
}

impl AtomicStatus {
    #[inline(always)]
    pub const fn new(status: Status) -> Self {
        // `as` conversion allowed here for const context.
        #[allow(clippy::as_conversions)]
        Self(Atomic::<u8>::new(status as u8))
    }

    #[inline(always)]
    pub fn load(&self, ordering: Ordering) -> Status {
        // Safety: We know that the inner integer must have been constructed from a
        // Status in the first place.
        unsafe { Status::unchecked_transmute_from(self.0.load(ordering)) }
    }

    #[inline(always)]
    pub fn store(&self, status: Status, ordering: Ordering) {
        self.0.store(u8::from(status), ordering);
    }

    #[inline(always)]
    pub fn compare_exchange(
        &self,
        old: Status,
        new: Status,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Status, Status> {
        // Safety:
        // A compare exchange will always return a value that was later stored into the
        // `Atomic<u8>`, but due to the invariant that it must be a valid `Status`, we
        // know that both Ok(_) and Err(_) will be safely transmutable.
        unsafe {
            match self
                .0
                .compare_exchange(u8::from(old), u8::from(new), success, failure)
            {
                Ok(ok) => Ok(Status::unchecked_transmute_from(ok)),
                Err(err) => Err(Status::unchecked_transmute_from(err)),
            }
        }
    }
}

pub struct Once<T> {
    status: AtomicStatus,
    data: UnsafeCell<MaybeUninit<T>>,
}

// Safety: `Once<T>` is required to maintain safety invariants.
unsafe impl<T: Send + Sync> Sync for Once<T> {}
// Safety: `Once<T>` is required to maintain safety invariants.
unsafe impl<T: Send> Send for Once<T> {}

impl<T> Once<T> {
    pub const fn new() -> Self {
        Self {
            status: AtomicStatus::new(Status::Incomplete),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Performs an initialization routine once and only once. The given closure
    /// will be executed if this is the first time [`Once::call_once`] has
    /// been called, and otherwise the routine will *not* be invoked.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// When this function returns, it is guaranteed that some initialization
    /// has run and completed (it may not be the closure specified). The
    /// returned pointer will point to the result from the closure that was run.
    ///
    /// # Panics
    ///
    /// This function will panic if the [`Once`] previously panicked while
    /// attempting to initialize. This is similar to the poisoning behaviour of
    /// `std::sync`'s primitives.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::util::sync::Once;
    ///
    /// static INIT: Once<usize> = Once::new();
    ///
    /// fn get_cached_val() -> usize {
    ///     *INIT.call_once(expensive_computation)
    /// }
    ///
    /// fn expensive_computation() -> usize {
    ///     // ...
    /// }
    /// ```
    pub fn call_once(&self, func: impl FnOnce() -> T) -> &T {
        match self.try_call_once(|| Ok::<T, core::convert::Infallible>(func())) {
            Ok(x) => x,
            Err(void) => match void {},
        }
    }

    /// This method is similar to [`Once::call_once`], but allows the given
    /// closure to fail, and leaves the [`Once`] in a uninitialized state if
    /// it does.
    ///
    /// This method will block the calling thread if another initialization
    /// routine is currently running.
    ///
    /// When this function returns without error, it is guaranteed that some
    /// initialization has run and completed (it may not be the closure
    /// specified). The returned reference will point to the result from the
    /// closure that was run.
    ///
    /// # Panics
    ///
    /// This function will panic if the [`Once`] previously panicked while
    /// attempting to initialize. This is similar to the poisoning behaviour of
    /// `std::sync`'s primitives.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::util::sync::Once;
    ///
    /// static INIT: Once<usize> = Once::new();
    ///
    /// fn get_cached_val() -> Result<usize, String> {
    ///     INIT.try_call_once(expensive_fallible_computation)
    ///         .map(|x| *x)
    /// }
    ///
    /// fn expensive_fallible_computation() -> Result<usize, String> {
    ///     // ...
    /// }
    /// ```
    pub fn try_call_once<F: FnOnce() -> Result<T, E>, E>(&self, f: F) -> Result<&T, E> {
        if let Some(value) = self.get() {
            Ok(value)
        } else {
            self.try_call_once_slow(f)
        }
    }

    #[cold]
    fn try_call_once_slow<E>(&self, func: impl FnOnce() -> Result<T, E>) -> Result<&T, E> {
        crate::interrupts::uninterruptable(|| {
            loop {
                let xchg = self.status.compare_exchange(
                    Status::Incomplete,
                    Status::Running,
                    Ordering::Acquire,
                    Ordering::Acquire,
                );

                match xchg {
                    Ok(_must_be_state_incomplete) => {
                        // Impl is defined after the match for readability
                    }

                    Err(Status::Panicked) => panic!("`Once::call_once` panicked"),

                    Err(Status::Running) => match self.poll() {
                        Some(v) => return Ok(v),
                        None => continue,
                    },

                    Err(Status::Complete) => {
                        return Ok({
                            // Safety: `self.status` is checked to be `Status::Complete`.
                            unsafe { self.force_get() }
                        });
                    }

                    Err(Status::Incomplete) => {
                        // The compare_exchange failed, so this shouldn't ever be reached,
                        // however if we decide to switch to compare_exchange_weak it will
                        // be safer to leave this here than hit an unreachable
                        continue;
                    }
                }

                // The compare-exchange succeeded, so we shall initialize it.

                // We use a guard (Finish) to catch panics caused by builder
                let finish = Finish {
                    status: &self.status,
                };

                let val = match func() {
                    Ok(val) => val,

                    Err(err) => {
                        // If an error occurs, clean up everything and leave.

                        core::mem::forget(finish);
                        self.status.store(Status::Incomplete, Ordering::Release);
                        return Err(err);
                    }
                };

                // Safety: `self.data` is not otherwise aliased, ensured by CAS exclusion.
                unsafe {
                    self.data.get().write(MaybeUninit::new(val));
                };

                // If there were to be a panic with unwind enabled, the code would short-circuit
                // and never reach the point where it writes the inner data. The destructor for
                // `Finish` will run, and poison the `Once` to ensure that other threads
                // accessing it do not exhibit unwanted behavior, if there were to be any
                // inconsistency in data structures caused by the panicking thread.
                //
                // However, `func` is expected in the general case not to panic. In that case,
                // we simply forget the guard, bypassing its destructor. We could theoretically
                // clear a flag instead, but this eliminates the call to the destructor at
                // compile time, and unconditionally poisons during an eventual panic, if
                // unwinding is enabled.
                core::mem::forget(finish);

                // `Ordering::Release` is required here, so that all memory accesses done in the
                // closure when initializing, become visible to other threads that perform
                // `Ordering::Acquire` loads.
                //
                // And, we also know that the changes this thread has done will not magically
                // disappear from our cache, so it does not need to be `Ordering::AcqRel`.
                self.status.store(Status::Complete, Ordering::Release);

                // Safety: `self.status` has been updated to be `Status::Complete`.
                return unsafe { Ok(self.force_get()) };
            }
        })
    }

    /// Get a reference to the initialized instance.
    ///
    /// # Safety
    ///
    /// - `self.status` must be `Status::Complete`.
    unsafe fn force_get(&self) -> &T {
        let ptr = self.data.get();

        // Safety: Caller is required to ensure `self.status` is `Status::Complete`,
        // which indicates that `self.data` will be fully initialized and convertible to
        // a reference.
        unsafe { MaybeUninit::assume_init_ref(ptr.as_ref_unchecked()) }
    }

    /// Returns a reference to the inner value if the [`Once`] has been
    /// initialized.
    pub fn get(&self) -> Option<&T> {
        // Just as with `self.poll`, `Ordering::Acquire`` is safe here because we want
        // to be able to see the non-atomic stores done when initializing, once
        // we have loaded and checked the status.
        match self.status.load(Ordering::Acquire) {
            Status::Complete => Some({
                // Safety: `self.status` is checked to be `Status::Complete`.
                unsafe { self.force_get() }
            }),
            _ => None,
        }
    }

    /// Like [`Once::get`], but will spin if the [`Once`] is in the
    /// process of being initialized. If initialization has not even begun,
    /// [`None`] will be returned.
    ///
    /// # Panics
    ///
    /// This function will panic if the [`Once`] previously panicked while
    /// attempting to initialize. This is similar to the poisoning behaviour of
    /// `std::sync`'s primitives.
    pub fn poll(&self) -> Option<&T> {
        loop {
            // `Ordering::Acquire` is safe here, because if the status is
            // `Status::Complete`, then we want to make sure that all memory accesses done
            // while initializing that value are visible when we return a reference to the
            // inner data after this load.
            match self.status.load(Ordering::Acquire) {
                Status::Incomplete => return None,

                Status::Running => {
                    // We spin for one loop.
                    core::hint::spin_loop();
                }

                Status::Complete => {
                    return Some({
                        // Safety: `self.status` is checked to be `Status::Complete`.
                        unsafe { self.force_get() }
                    });
                }

                Status::Panicked => panic!("Once previously poisoned by a panicked"),
            }
        }
    }
}
