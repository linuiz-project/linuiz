use core::ptr::NonNull;

#[repr(align(0x10))]
pub struct Stack<const SIZE: usize>([u8; SIZE]);

impl<const SIZE: usize> Stack<SIZE> {
    #[inline]
    pub const fn new() -> Self {
        Self([0u8; SIZE])
    }

    pub fn top(&self) -> NonNull<u8> {
        // Safety: Pointer is valid for the length of the slice.
        NonNull::new(unsafe { self.0.as_ptr().add(self.0.len()).cast_mut() }).unwrap()
    }
}

impl<const SIZE: usize> core::ops::Deref for Stack<SIZE> {
    type Target = [u8; SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
