use crate::mem::alloc::KERNEL_ALLOCATOR;
use alloc::{alloc::Allocator, boxed::Box};
use core::{mem::MaybeUninit, ptr::NonNull};

/// A process or kernel stack, aligned to a 16-byte boundary.
#[repr(align(0x10))]
#[derive(Clone, Copy)]
pub struct Stack<const N: usize>([MaybeUninit<u8>; N]);

impl<const N: usize> Default for Stack<N> {
    fn default() -> Self {
        Self([MaybeUninit::uninit(); N])
    }
}

impl<const N: usize> Stack<N> {
    pub fn new() -> Result<Box<Self>, core::alloc::AllocError> {
        let ptr = KERNEL_ALLOCATOR
            .allocate(core::alloc::Layout::new::<Self>())?
            .as_non_null_ptr()
            .cast::<Self>();

        // Safety: Memory was just allocated.
        let stack = unsafe { Box::from_non_null(ptr) };

        Ok(stack)
    }

    /// The top of the stack (traditional grow-down 'stack pointer').
    pub fn top(&self) -> NonNull<MaybeUninit<u8>> {
        let ptr = self.0.as_ptr().cast_mut();

        // Safety: `self.0` is valid for `MaybeUninit<u8>` for `N` bytes.
        let top_ptr = unsafe { ptr.byte_add(N) };

        // Safety: `self` cannot be null.
        unsafe { NonNull::new_unchecked(top_ptr) }
    }
}
