use crate::mem::alloc::KERNEL_ALLOCATOR;
use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};

/// A process or kernel stack, aligned to a 16-byte boundary.
#[repr(transparent)]
pub struct Stack<const N: usize>(NonNull<u8>);

impl<const N: usize> Stack<N> {
    /// Memory layout of a [`Stack`] of `N` bytes.
    fn layout() -> Layout {
        Layout::from_size_align(N, 0x10).expect("stack size is too large")
    }

    /// Allocates a new [`Stack`] with the kernel allocator, or returns [`AllocError`].
    pub fn allocate_new() -> Result<Self, AllocError> {
        KERNEL_ALLOCATOR.allocate(Self::layout()).map(|allocation| {
            let stack_bottom = allocation.as_non_null_ptr();
            // Safety: `[u8; N]` is valid for `NonNull<u8>` for len `N`.
            let stack_top = unsafe { stack_bottom.byte_add(N) };

            Self(stack_top)
        })
    }

    /// The top of the stack (traditional grow-down 'stack pointer').
    pub fn top(&self) -> NonNull<u8> {
        self.0
    }
}

impl<const N: usize> core::ops::Drop for Stack<N> {
    fn drop(&mut self) {
        // Safety: Stack is always `N` bytes large, and internal pointer points to
        //         the end of the original allocation.
        let stack_bottom = unsafe { self.0.byte_sub(N) };

        // Safety:
        //  - Layout is pre-defined by `Self::layout()`.
        //  - Allocation originated from kernel allocator.
        unsafe {
            KERNEL_ALLOCATOR.deallocate(stack_bottom, Self::layout());
        }
    }
}
