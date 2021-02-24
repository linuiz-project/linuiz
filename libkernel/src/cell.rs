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
