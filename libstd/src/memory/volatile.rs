use crate::{ReadOnly, ReadWrite};
use core::marker::PhantomData;

pub trait Volatile {}

pub trait VolatileAccess {}
impl VolatileAccess for ReadOnly {}
impl VolatileAccess for ReadWrite {}

#[repr(transparent)]
pub struct VolatileCell<T, V: VolatileAccess> {
    inner: core::cell::UnsafeCell<T>,
    phantom: PhantomData<V>,
}

impl<T, V: VolatileAccess> VolatileCell<T, V> {
    pub fn read(&self) -> T {
        unsafe { self.inner.get().read_volatile() }
    }

    pub fn as_ptr(&self) -> *const T {
        self.inner.get()
    }
}

impl<T> VolatileCell<T, ReadWrite> {
    pub fn write(&self, value: T) {
        unsafe { self.inner.get().write_volatile(value) };
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.inner.get()
    }
}

impl<T, V: VolatileAccess> Volatile for VolatileCell<T, V> {}

impl<T: core::fmt::Debug, V: VolatileAccess> core::fmt::Debug for VolatileCell<T, V> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("VolatileCell")
            .field(&self.read())
            .finish()
    }
}

#[repr(C)]
pub struct VolatileSplitPtr<T: Sized> {
    low: VolatileCell<u32, ReadWrite>,
    high: VolatileCell<u32, ReadWrite>,
    phantom: core::marker::PhantomData<T>,
}

impl<T: Sized> VolatileSplitPtr<T> {
    pub fn set_ptr(&self, ptr: *mut T) {
        let ptr_usize = ptr as usize;
        self.low.write(ptr_usize as u32);
        self.high.write((ptr_usize >> 32) as u32);
    }

    pub fn get_ptr(&self) -> *const T {
        ((self.low.read() as u64) | ((self.high.read() as u64) << 32)) as *const T
    }

    pub fn get_mut_ptr(&self) -> *mut T {
        ((self.low.read() as u64) | ((self.high.read() as u64) << 32)) as *mut T
    }
}
