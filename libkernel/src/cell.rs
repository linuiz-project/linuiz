use core::cell::UnsafeCell;

pub struct SyncRefCell<T>(UnsafeCell<T>);

unsafe impl<T> Send for SyncRefCell<T> {}
unsafe impl<T> Sync for SyncRefCell<T> {}
impl<T> SyncRefCell<T> {
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
}

impl<T> core::ops::Deref for SyncRefCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

pub struct SyncOnceCell<T> {
    inner_cell: core::lazy::OnceCell<T>,
}

unsafe impl<T> Send for SyncOnceCell<T> {}
unsafe impl<T> Sync for SyncOnceCell<T> {}
impl<T> SyncOnceCell<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            inner_cell: core::lazy::OnceCell::new(),
        }
    }

    #[inline]
    pub fn set(&self, obj: T) -> Result<(), T> {
        self.inner_cell.set(obj)
    }

    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.inner_cell.get()
    }

    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.inner_cell.get_mut()
    }
}
