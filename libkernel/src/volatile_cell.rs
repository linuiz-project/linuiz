#[derive(Clone, Copy)]
pub struct VolatileCell<T: Copy> {
    value: T,
}

impl<T: Copy> VolatileCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(&self.value) }
    }

    pub fn write(&mut self, new_value: T) {
        unsafe { core::ptr::write_volatile(&mut self.value, new_value) };
    }

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
