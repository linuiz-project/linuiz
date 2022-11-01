use crate::{AllocResult, GlobalAllocator};
use core::{
    alloc::{AllocError, Allocator, Layout},
    mem::size_of,
    num::NonZeroUsize,
    ptr::NonNull,
};

pub(crate) struct RawVec<T, A: Allocator = GlobalAllocator> {
    ptr: NonNull<T>,
    capacity: usize,
    allocator: A,
}

// SAFETY: Type is not thread-bound.
unsafe impl<T: Send, A: Allocator + Send> Send for RawVec<T, A> {}

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
    const T_IS_ZST: bool = size_of::<T>() == 0;

    pub const fn new_in(allocator: A) -> Self {
        Self { ptr: NonNull::dangling(), capacity: 0, allocator }
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
        if Self::T_IS_ZST || capacity == 0 {
            Ok(Self::new_in(allocator))
        } else {
            let layout = Layout::array::<T>(capacity).or(Err(AllocError))?;
            memory_overflow_guard(layout.size())?;
            let allocation = if zero_memory { allocator.allocate_zeroed(layout) } else { allocator.allocate(layout) }?;

            Ok(Self {
                // ### Safety: Pointer is known non-null from allocator.
                ptr: unsafe { NonNull::new_unchecked(allocation.as_non_null_ptr().as_ptr().cast()) },
                capacity,
                allocator,
            })
        }
    }

    #[inline]
    pub const fn ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }

    #[inline]
    pub const fn ptr_mut(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        if Self::T_IS_ZST {
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
        if Self::T_IS_ZST || self.capacity == 0 {
            None
        } else {
            Some((
                // ### Safety: If the above conditions weren't met, the pointer is valid.
                unsafe { NonNull::new_unchecked(self.ptr.as_ptr().cast()) },
                // ### Safety: If `capacity > 0`, then the layout will be valid.
                unsafe { Layout::array::<T>(self.capacity).unwrap_unchecked() },
            ))
        }
    }

    fn set_ptr_and_capacity(&mut self, ptr: NonNull<[u8]>, capacity: usize) {
        self.ptr = unsafe { NonNull::new_unchecked(ptr.as_non_null_ptr().as_ptr().cast()) };
        self.capacity = capacity;
    }

    /// Ensures that the buffer contains at least enough space to hold `len +
    /// additional` elements. If it doesn't already have enough capacity, will
    /// reallocate space up to the next power of two capacity to get amortized
    /// *O*(1) behavior.
    ///
    /// If `len` exceeds `self.capacity()`, this may fail to actually allocate
    /// the requested space. This is not really unsafe, but the unsafe
    /// code *you* write that relies on the behavior of this function may break.
    ///
    /// This is ideal for implementing a bulk-push operation like `extend`.
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

    pub fn reserve_exact(&mut self, len: usize, additional: NonZeroUsize) -> AllocResult<()> {
        if self.needs_to_grow(len, additional) {
            self.grow_exact(len, additional)
        } else {
            Ok(())
        }
    }

    pub fn reserve_one(&mut self, len: usize) -> AllocResult<()> {
        // ### Safety: Value is non-zero.
        self.grow_amortized(len, unsafe { NonZeroUsize::new_unchecked(1) })
    }

    const fn needs_to_grow(&self, len: usize, additional: NonZeroUsize) -> bool {
        additional.get() > self.capacity().wrapping_sub(len)
    }

    // This method is usually instantiated many times. So we want it to be as
    // small as possible, to improve compile times. But we also want as much of
    // its contents to be statically computable as possible, to make the
    // generated code run faster. Therefore, this method is carefully written
    // so that all of the code that depends on `T` is within it, while as much
    // of the code that doesn't depend on `T` as possible is in functions that
    // are non-generic over `T`.
    fn grow_amortized(&mut self, len: usize, additional: NonZeroUsize) -> AllocResult<()> {
        if Self::T_IS_ZST {
            return Err(AllocError);
        }

        let required_capacity = len.checked_add(additional.get()).ok_or(AllocError)?;
        let new_capacity = crate::next_capacity(required_capacity);
        let new_layout = Layout::array::<T>(new_capacity).or(Err(AllocError))?;
        let ptr = finish_grow(new_layout, self.current_memory(), &mut self.allocator)?;

        self.set_ptr_and_capacity(ptr, new_capacity);

        Ok(())
    }

    // The constraints on this method are much the same as those on
    // `grow_amortized`, but this method is usually instantiated less often so
    // it's less critical.
    fn grow_exact(&mut self, len: usize, additional: NonZeroUsize) -> AllocResult<()> {
        if Self::T_IS_ZST {
            return Err(AllocError);
        }

        let capacity = len.checked_add(additional.get()).ok_or(AllocError)?;
        let Ok(new_layout) = Layout::array::<T>(capacity) else { return Err(AllocError) };

        let ptr = finish_grow(new_layout, self.current_memory(), &mut self.allocator)?;
        self.set_ptr_and_capacity(ptr, capacity);

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

    match current_memory {
        Some((ptr, old_layout)) => {
            debug_assert_eq!(new_layout.align(), old_layout.align());

            // ### Safety: Allocator checks for alignment equality.
            unsafe { core::intrinsics::assume(new_layout.align() == old_layout.align()) };

            // ### Safety: Allocation data comes from the allocator itself.
            unsafe { allocator.grow(ptr, old_layout, new_layout) }
        }

        None => allocator.allocate_zeroed(new_layout),
    }
}

#[inline]
const fn memory_overflow_guard(allocation_size: usize) -> AllocResult<()> {
    if usize::BITS < 64 && allocation_size > (isize::MAX as usize) {
        Err(AllocError)
    } else {
        Ok(())
    }
}
