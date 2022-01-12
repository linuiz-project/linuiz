use core::sync::atomic::{AtomicUsize, Ordering};

pub struct AtomicQueue<'q, T> {
    head: AtomicUsize,
    tail: AtomicUsize,
    slice: alloc::boxed::Box<[core::mem::MaybeUninit<T>]>,
}

impl AtomicQueue<'_, T> {
    pub fn new(len: usize) -> Self {
        let slice = crate::alloc!(len).unwrap();

        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            slice: unsafe { slice.into_uninit_slice() },
        }
    }

    pub fn enqueue(&mut self, obj: T) -> Result<(), ()> {
        let mut prev_tail = usize::MAX;
        while prev_tail == usize::MAX {
            prev_tail = self.tail.swap(usize::MAX, Ordering::AcqRel);
            
        }


    }

    pub fn dequeue(&mut self) -> T {
        self.tail.compare_exchange(current, new, success, failure)
    }

    fn increment_head(&self) {

    }
}
