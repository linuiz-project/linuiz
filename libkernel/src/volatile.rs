use crate::{ReadOnly, ReadWrite};
use core::marker::PhantomData;

pub trait VolatileAccess {}
impl VolatileAccess for ReadOnly {}
impl VolatileAccess for ReadWrite {}

pub struct Volatile<T, V: VolatileAccess> {
    ptr: *mut T,
    phantom: PhantomData<V>,
}

impl<T, V: VolatileAccess> Volatile<T, V> {
    pub fn read(&self) -> T {
        unsafe { self.ptr.read_volatile() }
    }
}

impl<T> Volatile<T, ReadOnly> {
    pub unsafe fn new(ptr: *const T) -> Self {
        Self {
            ptr: ptr as *mut T,
            phantom: PhantomData,
        }
    }
}

impl<T> Volatile<T, ReadWrite> {
    pub unsafe fn new(ptr: *mut T) -> Self {
        Self {
            ptr,
            phantom: PhantomData,
        }
    }

    pub fn write(&mut self, value: T) {
        unsafe { self.ptr.write_volatile(value) };
    }
}

#[repr(transparent)]
pub struct VolatileCell<T, V: VolatileAccess> {
    obj: T,
    phantom: PhantomData<V>,
}

impl<T, V: VolatileAccess> VolatileCell<T, V> {
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile((&self.obj) as *const T) }
    }
}

impl<T> VolatileCell<T, ReadWrite> {
    pub fn write(&mut self, value: T) {
        unsafe {
            core::ptr::write_volatile((&mut self.obj) as *mut T, value);
        }
    }
}
