use crate::{raw_vec::RawVec, AllocResult, GlobalAllocator};
use core::alloc::Allocator;

pub struct Deque<T, A: Allocator = GlobalAllocator> {
    buffer: RawVec<T, A>,
    head: usize,
    tail: usize,
}

impl<T> Deque<T, GlobalAllocator> {
    pub const fn new() -> Self {
        Self { buffer: RawVec::new(), head: 0, tail: 0 }
    }

    pub fn with_capacity(capacity: usize) -> AllocResult<Self> {
        RawVec::with_capacity(capacity).map(|buffer| Self { buffer, head: 0, tail: 0 })
    }
}

impl<T, A: Allocator> Deque<T, A> {
    pub const fn new_in(allocator: A) -> Self {
        Self { buffer: RawVec::new_in(allocator), head: 0, tail: 0 }
    }

    #[inline]
    pub fn with_capacity_in(capacity: usize, allocator: A) -> AllocResult<Self> {
        RawVec::with_capacity_in(capacity, allocator).map(|buffer| Self { buffer, head: 0, tail: 0 })
    }

    #[inline]
    const fn ptr(&self) -> *const T {
        self.buffer.ptr()
    }

    #[inline]
    const unsafe fn ptr_mut(&self) -> *mut T {
        self.buffer.ptr_mut()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.tail - self.head
    }

    /// Returns `true` if the collection has no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// Returns `true` if the collection is at full capacity.
    #[inline]
    pub const fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    #[inline]
    const fn capacity_index(&self, value: usize) -> usize {
        debug_assert!(self.capacity() == 0 || self.capacity().is_power_of_two());
        value & self.capacity().saturating_sub(1)
    }

    pub fn push_front(&mut self, element: T) -> AllocResult<()> {
        if self.is_full() {
            self.grow_one()?;
        }

        debug_assert!(self.capacity() > 0);
        let head_index = self.capacity_index(self.head);
        self.head = self.head.wrapping_sub(1);

        // ### Safety: pointer is guaranteed to be valid by the above checks.
        unsafe { self.ptr_mut().add(head_index).write(element) };

        Ok(())
    }

    pub fn push_back(&mut self, element: T) -> AllocResult<()> {
        if self.is_full() {
            self.grow_one()?;
        }

        debug_assert!(self.capacity() > 0);
        let tail_index = self.capacity_index(self.tail);
        self.tail = self.tail.wrapping_add(1);

        // ### Safety: pointer is guaranteed to be valid by the above checks.
        unsafe { self.ptr_mut().add(tail_index).write(element) };

        Ok(())
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let head_index = self.capacity_index(self.head);
            self.head = self.head.wrapping_add(1);

            // ### Safety: pointer is guaranteed to be valid by the above checks.
            Some(unsafe { self.ptr().add(head_index).read() })
        }
    }

    /// Double the buffer size. This method is inline(never), so we expect it to only
    /// be called in cold paths.
    #[inline(never)]
    fn grow_one(&mut self) -> AllocResult<()> {
        debug_assert!(self.is_full());

        let old_capacity = self.capacity();
        // ### Safety: value is non-zero.
        self.buffer.reserve_one(old_capacity)?;
        debug_assert_eq!(self.capacity(), crate::next_capacity(old_capacity.wrapping_add(1)));
        // ### Safety: old capacity is known valid.
        unsafe { self.handle_capacity_increase(old_capacity) };

        Ok(())
    }

    unsafe fn handle_capacity_increase(&mut self, old_capacity: usize) {
        debug_assert!(old_capacity == 0 || old_capacity.is_power_of_two());
        let old_head_index = self.head & old_capacity.saturating_sub(1);
        let old_tail_index = self.tail & old_capacity.saturating_sub(1);

        if self.len() == 0 {
            // distinguish 0 capacity grow for the next if condition
        } else if old_tail_index <= old_head_index {
            // copy the lower elements from the old memory to the lower bound of the new memory.
            unsafe {
                let added_capacity = self.capacity() - old_capacity;
                // Adjust head and tail to maintain their wrapping characteristic from the old capacity.
                self.head += added_capacity;
                self.tail += added_capacity;

                let old_head_ptr = self.ptr().add(old_head_index);
                let new_head_ptr = self.ptr_mut().add(old_head_index + added_capacity);

                new_head_ptr.copy_from_nonoverlapping(old_head_ptr, old_capacity - old_head_index);
            }
        }

        debug_assert!(self.head < self.capacity());
        debug_assert!(self.len() < self.capacity());
        debug_assert_eq!(self.capacity().count_ones(), 1);
    }
}

impl<T: core::fmt::Debug, A: Allocator> Deque<T, A> {
    #[cfg(test)]
    pub fn print_contents(&self) {
        // ### Safety: Slice is valid for range so long as vec is valid for range.
        std::println!(
            "HEAD {}({}) TAIL {}({}) {:?}",
            self.head,
            self.capacity_index(self.head),
            self.tail,
            self.capacity_index(self.tail),
            unsafe { core::slice::from_raw_parts(self.ptr(), self.capacity()) }
        )
    }
}

#[test]
pub fn push_pop_1000() {
    let mut deque = Deque::new_in(std::alloc::Global);
    let alloc_range = 0..1000;

    for i in alloc_range.clone() {
        deque.push_back(i).unwrap();
    }

    for i in alloc_range.clone() {
        assert_eq!(deque.pop_front(), Some(i));
    }
}

#[test]
pub fn push_pop_laddering() {
    let mut deque = Deque::new_in(std::alloc::Global);

    for i1 in 0..2 {
        for i in 0..4 {
            deque.push_back(i).unwrap();
        }

        for _ in 0..2 {
            deque.pop_front().unwrap();
        }
    }

    for i in 0..4 {
        assert_eq!(deque.pop_front(), Some(i));
    }
}
