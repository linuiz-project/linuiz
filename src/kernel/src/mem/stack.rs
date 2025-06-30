use core::ptr::NonNull;

#[repr(align(0x10))]
#[derive(FromZeros)]
pub struct Stack<const SIZE: usize>([u8; SIZE]);

impl<const SIZE: usize> Stack<SIZE> {
    pub fn top(&self) -> NonNull<u8> {
        // Safety: Pointer is valid for the length of the slice.
        NonNull::new(unsafe { self.0.as_ptr().add(self.0.len()).cast_mut() }).unwrap()
    }
}
