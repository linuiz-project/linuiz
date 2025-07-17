use core::{mem::MaybeUninit, ptr::NonNull};

/// A process or kernel stack, aligned to a 16-byte boundary.
#[repr(C, align(0x10))]
#[derive(FromBytes, Clone, Copy)]
pub struct Stack<const N: usize>([MaybeUninit<u8>; N]);

impl<const N: usize> Stack<N> {
    /// The top of the stack (traditional grow-down 'stack pointer').
    pub fn top(&self) -> NonNull<MaybeUninit<u8>> {
        let ptr = self.0.as_ptr().cast_mut();

        // Safety: `self.0` is valid for `MaybeUninit<u8>` for `N` bytes.
        let top_ptr = unsafe { ptr.byte_add(N) };

        // Safety: `self` cannot be null.
        unsafe { NonNull::new_unchecked(top_ptr) }
    }
}
