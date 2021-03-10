use core::cell::UnsafeCell;

pub struct SyncCell<T> {
    obj: Option<T>,
}

unsafe impl<T> Send for SyncCell<T> {}
unsafe impl<T> Sync for SyncCell<T> {}

impl<T> SyncCell<T> {
    pub const fn new() -> Self {
        Self { obj: None }
    }

    pub fn set(&mut self, obj: T) {
        self.obj = Some(obj);
    }

    pub fn get(&self) -> Option<&T> {
        self.obj.as_ref()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.obj.as_mut()
    }
}

pub struct SyncRefCell<T> {
    inner_cell: UnsafeCell<Option<T>>,
}

unsafe impl<T> Send for SyncRefCell<T> {}
unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub const fn new() -> Self {
        Self {
            inner_cell: UnsafeCell::new(None),
        }
    }

    pub fn set(&self, obj: T) {
        unsafe { *self.inner_cell.get() = Some(obj) }
    }

    pub fn get(&self) -> &Option<T> {
        unsafe { &*self.inner_cell.get() }
    }

    pub fn get_mut(&self) -> &mut Option<T> {
        unsafe { &mut *self.inner_cell.get() }
    }
}
