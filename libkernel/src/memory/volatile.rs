use crate::{ReadOnly, ReadWrite, WriteOnly};
use core::marker::PhantomData;

pub trait Volatile {}

pub trait VolatileAccess {}
impl VolatileAccess for ReadOnly {}
impl VolatileAccess for WriteOnly {}
impl VolatileAccess for ReadWrite {}

#[repr(transparent)]
pub struct VolatileCell<T, V: VolatileAccess>(core::cell::UnsafeCell<T>, PhantomData<V>);

impl<T, V: VolatileAccess> VolatileCell<T, V> {
    /// Returns a new `VolatileCell` containing the given value.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self(core::cell::UnsafeCell::new(value), PhantomData)
    }
}

impl<T> VolatileCell<T, ReadOnly> {
    #[inline]
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.0.get()
    }
}

impl<T> VolatileCell<T, WriteOnly> {
    #[inline]
    pub fn write(&self, value: T) {
        unsafe { self.0.get().write_volatile(value) };
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.0.get()
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.0.get()
    }
}

impl<T> VolatileCell<T, ReadWrite> {
    #[inline]
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }

    #[inline]
    pub fn write(&self, value: T) {
        unsafe { self.0.get().write_volatile(value) };
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.0.get()
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.0.get()
    }
}

impl<T, V: VolatileAccess> Volatile for VolatileCell<T, V> {}

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
