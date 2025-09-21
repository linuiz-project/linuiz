pub mod elf;
pub mod math;
pub mod sync;

mod array_str;
pub use array_str::*;

pub trait InteriorBorrow {
    type RefType<'a, T>
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T;
}

pub struct SharedBorrow;
impl InteriorBorrow for SharedBorrow {
    type RefType<'a, T>
        = &'a T
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}

pub struct ExclusiveBorrow;
impl InteriorBorrow for ExclusiveBorrow {
    type RefType<'a, T>
        = &'a mut T
    where
        T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        r
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    major: u8,
    minor: u8,
    patch: u8,
}

impl Version {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn major(self) -> u8 {
        self.major
    }

    pub fn minor(self) -> u8 {
        self.minor
    }

    pub fn patch(self) -> u8 {
        self.patch
    }
}
