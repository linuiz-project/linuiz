#[derive(Clone, Copy)]
pub struct VolatileCell<T: Copy> {
    value: T,
}

impl<T: Copy> VolatileCell<T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    #[inline]
    pub fn write(&mut self, new_value: T) {
        unsafe { core::ptr::write_volatile(&mut self.value, new_value) };
    }

    #[inline]
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(&self.value) }
    }

    #[inline]
    pub fn update<F>(&mut self, update_fn: F)
    where
        F: FnOnce(T) -> T,
    {
        self.write(update_fn(self.read()))
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

impl<T: Copy> core::fmt::Debug for VolatileCell<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("VolatileCell").field(&*self).finish()
    }
}
