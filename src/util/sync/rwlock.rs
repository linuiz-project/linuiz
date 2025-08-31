use crate::interrupts::uninterruptable;
use core::{
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{Atomic, Ordering},
};

const READER: usize = 1 << 1;
const WRITER: usize = 1 << 0;

/// A lock that provides data access to either one writer or many readers.
///
/// This lock behaves in a similar manner to its namesake `std::sync::RwLock`
/// but uses spinning for synchronisation instead. Unlike its namespace, this
/// lock does not track lock poisoning.
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. This lock typically allows binding of the underlying data,
/// with [`RwLock::with_shared`] being shared and [`RwLock::with_exclusive`]
/// being exclusive.
///
/// The type parameter `T` represents the data that this lock protects. It is
/// required that `T` satisfies `Send` to be shared across tasks and `Sync` to
/// allow concurrent access through readers. The RAII guards returned from the
/// locking methods implement [`Deref`][core::ops::Deref] (and
/// [`DerefMut`][core::ops::DerefMut] for the [`RwLock::with_exclusive`] method)
/// to allow access to the contained of the lock.
///
/// Based on Facebook's
/// [`folly/RWSpinLock.h`](https://github.com/facebook/folly/blob/a0394d84f2d5c3e50ebfd0566f9d3acb52cfab5a/folly/synchronization/RWSpinLock.h).
/// This implementation is unfair to writers - if the lock always has readers,
/// then no writers will ever get a chance. If the lock is that highly contended
/// and writes are crucial then this implementation may be a poor choice.
pub struct RwLock<T: ?Sized> {
    lock: Atomic<usize>,
    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    /// Creates a new [`RwLock`] [spin lock](https://en.m.wikipedia.org/wiki/Spinlock) wrapping the supplied data.
    pub fn new(data: T) -> Self {
        Self {
            lock: Atomic::<usize>::new(0),
            data: UnsafeCell::new(data),
        }
    }
}

// Safety: Same unsafe impls as `std::sync::RwLock`.
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
// Safety: Same unsafe impls as `std::sync::RwLock`.
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T: ?Sized> RwLock<T> {
    fn data_ptr(&self) -> NonNull<T> {
        let ptr = self.data.get();
        // Safety: `ptr` comes from an occupied `UnsafeCell<T>`.
        unsafe { NonNull::new_unchecked(ptr) }
    }

    /// Locks this [`RwLock`] with shared access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns. This method does not provide any guarantees with
    /// respect to the ordering of whether contentious readers or writers will
    /// acquire the lock first.
    pub fn with_shared<'a, U>(&'a self, func: impl FnOnce(&'a T) -> U) -> U {
        loop {
            // An arbitrary cap that allows us to catch overflows long before they happen
            const MAX_READERS: usize = usize::MAX / READER / 2;

            let value = self.lock.fetch_add(READER, Ordering::Acquire);

            if value > MAX_READERS * READER {
                self.lock.fetch_sub(READER, Ordering::Relaxed);
                panic!("`RwLock` has too many lock readers, cannot safely proceed");
            }

            if (value & WRITER) == 0 {
                // Using a block here ensures the shared borrow of `data` does not escape.
                let func_value = {
                    // Safety: `self.data` is atomically checked to allow shared access.
                    let data = unsafe { self.data.as_ref_unchecked() };

                    func(data)
                };

                // Release the shared borrow.
                self.lock.fetch_sub(READER, Ordering::Release);

                return func_value;
            }

            // Lock is taken, undo.
            self.lock.fetch_sub(READER, Ordering::Release);

            core::hint::spin_loop();
        }
    }

    /// Lock this [`RwLock`] with exclusive access, blocking the current thread
    /// until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    pub fn with_exclusive<'a, U>(&'a self, func: impl FnOnce(&'a mut T) -> U) -> U {
        uninterruptable(|| {
            loop {
                if self
                    .lock
                    .compare_exchange_weak(0, WRITER, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    // Using a block here ensures the exlusive borrow of `data` does not escape.
                    let func_value = {
                        // Safety: `self.data` is atomically checked to allow exclusive access.
                        let data = unsafe { self.data.as_mut_unchecked() };

                        func(data)
                    };

                    // Release the exclusive borrow.
                    self.lock.fetch_and(!WRITER, Ordering::Release);

                    return func_value;
                }

                core::hint::spin_loop();
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, mpsc::channel},
        thread,
    };

    type RwLock<T> = super::RwLock<T>;

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let l = RwLock::new(());
        l.with_shared(|a| *a);
        l.with_exclusive(|a| *a);
        l.with_shared(|a| *a);
        l.with_exclusive(|a| *a);
    }

    #[test]
    fn test_rw_arc() {
        let arc = Arc::new(RwLock::new(0));
        let arc2 = arc.clone();
        let (tx, rx) = channel();

        let t = thread::spawn(move || {
            arc2.with_exclusive(|lock| {
                for _ in 0..10 {
                    let tmp = *lock;
                    *lock = -1;
                    thread::yield_now();
                    *lock = tmp + 1;
                }

                tx.send(()).unwrap();
            });
        });

        // Readers try to catch the writer in the act
        let mut children = Vec::new();
        for _ in 0..5 {
            let arc3 = arc.clone();

            children.push(thread::spawn(move || {
                arc3.with_shared(|lock| {
                    assert!(*lock >= 0);
                });
            }));
        }

        // Wait for children to pass their asserts
        for r in children {
            assert!(r.join().is_ok());
        }

        // Wait for writer to finish
        rx.recv().unwrap();
        arc.with_shared(|lock| {
            assert_eq!(*lock, 10);
        });

        assert!(t.join().is_ok());
    }

    #[test]
    fn test_rw_access_in_unwind() {
        let arc = Arc::new(RwLock::new(1));
        let arc2 = arc.clone();

        let _ = thread::spawn(move || {
            struct Unwinder {
                i: Arc<RwLock<isize>>,
            }

            impl Drop for Unwinder {
                fn drop(&mut self) {
                    self.i.with_exclusive(|lock| {
                        *lock += 1;
                    });
                }
            }

            let _u = Unwinder { i: arc2 };

            panic!();
        })
        .join();

        arc.with_shared(|lock| {
            assert_eq!(*lock, 2);
        });
    }

    #[test]
    fn test_rwlock_unsized() {
        let rw: &RwLock<[i32]> = &RwLock::new([1, 2, 3]);

        rw.with_exclusive(|b| {
            b[0] = 4;
            b[2] = 5;
        });

        let comp: &[i32] = &[4, 2, 5];
        rw.with_shared(|lock| {
            assert_eq!(lock, comp);
        });
    }
}
