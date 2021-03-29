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
    obj: Option<T>,
}

unsafe impl<T> Send for SyncRefCell<T> {}
unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub const fn empty() -> Self {
        Self { obj: None }
    }

    pub const fn new(obj: T) -> Self {
        Self { obj: Some(obj) }
    }

    unsafe fn obj_ptr(&self) -> *mut Option<T> {
        (&self.obj) as *const _ as *mut _
    }

    pub fn set(&self, obj: T) {
        unsafe { *self.obj_ptr() = Some(obj) }
    }

    pub fn borrow<'a>(&'a self) -> Option<&'a T> {
        unsafe { (&*self.obj_ptr()).as_ref() }
    }

    pub fn borrow_mut<'a>(&'a self) -> Option<&'a mut T> {
        unsafe { (&mut *self.obj_ptr()).as_mut() }
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

#[derive(Clone, Copy)]
pub struct VolatileCell<T: Copy> {
    value: T,
}

impl<T: Copy> VolatileCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Performs a volatile read on the contained value.
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(&self.value) }
    }

    /// Performs a volatile write on the contained value.
    pub fn write(&mut self, new_value: T) {
        unsafe { core::ptr::write_volatile(&mut self.value, new_value) };
    }

    /// Performs a volatile read on the contained value, and passes
    /// the contained value to an update function, then performing
    /// a volatile write with the updated value.
    pub fn update<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut T),
    {
        let mut value = self.read();
        update_fn(&mut value);
        self.write(value);
    }
}

impl<T: Copy> core::ops::Deref for VolatileCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Copy> core::ops::DerefMut for VolatileCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Copy + core::fmt::Debug> core::fmt::Debug for VolatileCell<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("VolatileCell")
            .field(&self.value)
            .finish()
    }
}
