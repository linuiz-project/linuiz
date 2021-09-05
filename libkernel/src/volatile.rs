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

    pub fn borrow(&self) -> &T {
        unsafe { &*self.ptr }
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

    pub fn borrow_mut(&self) -> &mut T {
        unsafe { &mut *self.ptr }
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

    pub fn as_ptr(&self) -> *const T {
        self.borrow() as *const T
    }

    pub fn borrow(&self) -> &T {
        &self.obj
    }
}

impl<T> VolatileCell<T, ReadWrite> {
    pub fn write(&mut self, value: T) {
        unsafe {
            core::ptr::write_volatile((&mut self.obj) as *mut T, value);
        }
    }

    pub fn as_readonly(self) -> VolatileCell<T, ReadOnly> {
        VolatileCell::<T, ReadOnly> {
            obj: self.obj,
            phantom: PhantomData,
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.borrow_mut() as *mut T
    }

    pub fn borrow_mut(&mut self) -> &mut T {
        &mut self.obj
    }
}

#[repr(C)]
pub struct VolatileSplitPtr<T: Sized> {
    low: VolatileCell<u32, ReadWrite>,
    high: VolatileCell<u32, ReadWrite>,
    phantom: core::marker::PhantomData<T>,
}

impl<T: Sized> VolatileSplitPtr<T> {
    pub fn set_ptr(&mut self, ptr: *mut T) {
        let ptr_usize = ptr as usize;
        self.low.write(ptr_usize as u32);
        self.high.write((ptr_usize >> 32) as u32);
    }

    pub fn get_ptr(&self) -> *const T {
        ((self.low.read() as u64) | ((self.high.read() as u64) << 32)) as *const T
    }

    pub fn get_mut_ptr(&mut self) -> *mut T {
        ((self.low.read() as u64) | ((self.high.read() as u64) << 32)) as *mut T
    }
}
