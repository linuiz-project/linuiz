use core::cell::UnsafeCell;

pub struct SyncCell<T> {
    obj: Option<T>,
}

unsafe impl<T> Send for SyncCell<T> {}
unsafe impl<T> Sync for SyncCell<T> {}

impl<T> SyncCell<T> {
    pub const fn empty() -> Self {
        Self { obj: None }
    }

    pub const fn new(obj: T) -> Self {
        Self { obj: Some(obj) }
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
    cell: UnsafeCell<Option<T>>,
}

unsafe impl<T> Send for SyncRefCell<T> {}
unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub const fn empty() -> Self {
        Self {
            cell: UnsafeCell::new(None),
        }
    }

    pub const fn new(obj: T) -> Self {
        Self {
            cell: UnsafeCell::new(Some(obj)),
        }
    }

    pub fn set(&self, obj: T) {
        unsafe { *self.cell.get() = Some(obj) }
    }

    pub fn borrow<'a>(&'a self) -> Option<&'a T> {
        unsafe { self.cell.get().as_ref().unwrap().as_ref() }
    }

    pub fn borrow_mut<'a>(&'a self) -> Option<&'a mut T> {
        unsafe { self.cell.get().as_mut().unwrap().as_mut() }
    }
}

pub struct SyncOnceCell<T> {
    inner_cell: core::lazy::OnceCell<T>,
}

unsafe impl<T> Send for SyncOnceCell<T> {}
unsafe impl<T> Sync for SyncOnceCell<T> {}

impl<T> SyncOnceCell<T> {
    pub const fn new() -> Self {
        Self {
            inner_cell: core::lazy::OnceCell::new(),
        }
    }

    pub fn set(&self, obj: T) -> Result<(), T> {
        self.inner_cell.set(obj)
    }

    pub fn get(&self) -> Option<&T> {
        self.inner_cell.get()
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.inner_cell.get_mut()
    }
}
