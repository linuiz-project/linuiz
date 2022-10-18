use crate::{AllocResult, GlobalAllocator};
use core::{
    alloc::{AllocError, Allocator, Layout},
    mem::size_of,
    num::NonZeroUsize,
    ptr::{NonNull, Unique},
};

pub(crate) struct RawVec<T, A: Allocator = GlobalAllocator> {
    ptr: Unique<T>,
    capacity: usize,
    allocator: A,
}

impl<T> RawVec<T, GlobalAllocator> {
    pub const fn new() -> Self {
        Self::new_in(GlobalAllocator)
    }

    pub fn with_capacity(capacity: usize) -> AllocResult<Self> {
        Self::with_capacity_in(capacity, GlobalAllocator)
    }

    pub fn with_capacity_zeroed(capacity: usize) -> AllocResult<Self> {
        Self::with_capacity_zeroed_in(capacity, GlobalAllocator)
    }
}

impl<T, A: Allocator> RawVec<T, A> {
    const IS_ZST: bool = size_of::<T>() == 0;

    pub const fn new_in(allocator: A) -> Self {
        Self { ptr: Unique::dangling(), capacity: 0, allocator }
    }

    #[inline]
    pub fn with_capacity_in(capacity: usize, allocator: A) -> AllocResult<Self> {
        Self::allocate_in(capacity, false, allocator)
    }

    #[inline]
    pub fn with_capacity_zeroed_in(capacity: usize, allocator: A) -> AllocResult<Self> {
        Self::allocate_in(capacity, true, allocator)
    }

    fn allocate_in(capacity: usize, zero_memory: bool, allocator: A) -> AllocResult<Self> {
        if Self::IS_ZST || capacity == 0 {
            Ok(Self::new_in(allocator))
        } else {
            let layout = Layout::array::<T>(capacity).or(Err(AllocError))?;
            memory_overflow_guard(layout.size())?;
            let allocation = if zero_memory { allocator.allocate_zeroed(layout) } else { allocator.allocate(layout) }?;

            Ok(Self {
                // SAFETY: Pointer is known non-null from allocator.
                ptr: unsafe { Unique::new_unchecked(allocation.as_non_null_ptr().as_ptr().cast()) },
                capacity,
                allocator,
            })
        }
    }

    #[inline]
    pub const fn ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        if Self::IS_ZST {
            usize::MAX
        } else {
            self.capacity
        }
    }

    #[inline]
    pub const fn allocator(&self) -> &A {
        &self.allocator
    }

    fn current_memory(&self) -> Option<(NonNull<u8>, Layout)> {
        if Self::IS_ZST || self.capacity == 0 {
            None
        } else {
            Some((
                // SAFETY: If the above conditions weren't met, the pointer is valid.
                unsafe { NonNull::new_unchecked(self.ptr.as_ptr().cast()) },
                // SAFETY: If `capacity > 0`, then the layout will be valid.
                unsafe { Layout::array::<T>(self.capacity).unwrap_unchecked() },
            ))
        }
    }

    fn set_ptr_and_capacity(&mut self, ptr: NonNull<[u8]>, capacity: usize) {
        self.ptr = unsafe { Unique::new_unchecked(ptr.as_non_null_ptr().as_ptr().cast()) };
        self.capacity = capacity;
    }

    #[inline]
    pub fn reserve(&mut self, len: usize, additional: NonZeroUsize) -> AllocResult<()> {
        // This logic is extraced into a non-generic function to avoid bloating the code-size of `reserve`.
        #[cold]
        fn do_reserve_and_handle<T, A: Allocator>(
            raw_vec: &mut RawVec<T, A>,
            len: usize,
            additional: NonZeroUsize,
        ) -> AllocResult<()> {
            raw_vec.grow_amortized(len, additional)
        }

        if self.needs_to_grow(len, additional) {
            do_reserve_and_handle(self, len, additional)
        } else {
            Ok(())
        }
    }

    pub fn reserve_for_push(&mut self, len: usize) -> AllocResult<()> {
        // SAFETY: Value is non-zero.g
        self.grow_amortized(len, unsafe { NonZeroUsize::new_unchecked(1) })
    }

    const fn needs_to_grow(&self, len: usize, additional: NonZeroUsize) -> bool {
        additional.get() > self.capacity().wrapping_sub(len)
    }

    fn grow_amortized(&mut self, len: usize, additional: NonZeroUsize) -> AllocResult<()> {
        if Self::IS_ZST {
            return Err(AllocError);
        }

        let required_capacity = len.checked_add(additional.get()).ok_or(AllocError)?;
        let new_capacity = core::cmp::max(self.capacity.next_power_of_two(), required_capacity);
        let new_layout = Layout::array::<T>(new_capacity).or(Err(AllocError))?;
        let ptr = finish_grow(new_layout, self.current_memory(), &mut self.allocator)?;

        self.set_ptr_and_capacity(ptr, new_capacity);

        Ok(())
    }
}

#[inline(never)]
fn finish_grow<A: Allocator>(
    new_layout: Layout,
    current_memory: Option<(NonNull<u8>, Layout)>,
    allocator: &mut A,
) -> AllocResult<NonNull<[u8]>> {
    memory_overflow_guard(new_layout.size())?;

    current_memory.ok_or(AllocError).and_then(|(ptr, old_layout)| {
        debug_assert_eq!(new_layout.align(), old_layout.align());

        // SAFETY: Allocator checks for alignment equality.
        unsafe { core::intrinsics::assume(new_layout.align() == old_layout.align()) };
        // SAFETY: Allocation data comes from the allocator itself.
        unsafe { allocator.grow(ptr, old_layout, new_layout) }
    })
}

#[inline]
const fn memory_overflow_guard(allocation_size: usize) -> AllocResult<()> {
    if usize::BITS < 64 && allocation_size > (isize::MAX as usize) {
        Err(AllocError)
    } else {
        Ok(())
    }
}
