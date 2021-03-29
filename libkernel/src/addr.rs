pub mod addr_ty {
    pub trait AddressType {}

    pub enum Physical {}
    impl AddressType for Physical {}

    pub enum Virtual {}
    impl AddressType for Virtual {}
}

use crate::addr_ty::*;
use core::marker::PhantomData;

pub const VADDR_HW_MAX: usize = 0x1000000000000;

#[repr(transparent)]
pub struct Address<T: AddressType> {
    value: usize,
    phantom: PhantomData<T>,
}

impl<T: AddressType> Address<T> {
    pub const fn zero() -> Self {
        Self {
            value: 0,
            phantom: PhantomData,
        }
    }

    pub const unsafe fn new_unsafe(addr: usize) -> Self {
        Self {
            value: addr,
            phantom: PhantomData,
        }
    }

    pub const fn as_usize(&self) -> usize {
        self.value
    }

    pub const fn is_null(&self) -> bool {
        self.value == 0
    }

    pub const fn align_up(self, alignment: usize) -> Self {
        Self {
            value: crate::align_up(self.value, alignment),
            phantom: PhantomData,
        }
    }

    pub const fn align_down(self, alignment: usize) -> Self {
        Self {
            value: crate::align_down(self.value, alignment),
            phantom: PhantomData,
        }
    }

    pub const fn is_aligned(&self, alignment: usize) -> bool {
        crate::align_down(self.value, alignment) == self.value
    }
}

impl<T: AddressType> Copy for Address<T> {}
impl<T: AddressType> Clone for Address<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            phantom: PhantomData,
        }
    }
}

impl<T: AddressType> Eq for Address<T> {}
impl<T: AddressType> PartialEq for Address<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: AddressType> Ord for Address<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        if self.value < other.value {
            Ordering::Less
        } else if self.value > other.value {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl<T: AddressType> PartialOrd for Address<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Address<Physical> {
    pub const fn new(addr: usize) -> Self {
        match addr >> 52 {
            0 => Self {
                value: addr,
                phantom: PhantomData,
            },
            _ => panic!("given address is not canonical (bits 52..64 contain data)"),
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        Self {
            value: addr & 0xFFFFFFFFFFFFF,
            phantom: PhantomData,
        }
    }

    pub const fn frame_index(&self) -> usize {
        self.as_usize() / 0x1000
    }

    pub const fn is_canonical(&self) -> bool {
        (self.value >> 52) == 0
    }
}

impl core::ops::Add<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value.checked_add(rhs.value).unwrap())
    }
}

impl core::ops::Add<usize> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.value.checked_add(rhs).unwrap())
    }
}

impl core::ops::AddAssign<usize> for Address<Physical> {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs
    }
}

impl core::ops::Sub<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value.checked_sub(rhs.value).unwrap())
    }
}

impl core::ops::Sub<usize> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.value.checked_sub(rhs).unwrap())
    }
}

impl core::ops::SubAssign<usize> for Address<Physical> {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs
    }
}

impl core::fmt::Debug for Address<Physical> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Address<Physical>")
            .field(&(self.value as *mut core::ffi::c_void))
            .finish()
    }
}

impl Address<Virtual> {
    pub const fn new(addr: usize) -> Self {
        match addr >> 47 {
            0 | 0x1FFFF => Self {
                value: addr,
                phantom: PhantomData,
            },
            1 => Self::new_truncate(addr),
            _ => panic!("given address is not canonical (bits 48..64 contain data)"),
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        Self {
            value: (((addr << 16) as isize) >> 16) as usize,
            phantom: PhantomData,
        }
    }

    pub const fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(unsafe { ptr as usize })
    }

    pub const fn page_index(&self) -> usize {
        self.as_usize() / 0x1000
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        self.value as *const T
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.value as *mut T
    }

    pub const fn page_offset(&self) -> usize {
        self.value & 0xFFF
    }

    pub const fn p1_index(&self) -> usize {
        (self.value >> 12) & 0x1FF
    }

    pub const fn p2_index(&self) -> usize {
        (self.value >> 12 >> 9) & 0x1FF
    }

    pub const fn p3_index(&self) -> usize {
        (self.value >> 12 >> 9 >> 9) & 0x1FF
    }

    pub const fn p4_index(&self) -> usize {
        (self.value >> 12 >> 9 >> 9 >> 9) & 0x1FF
    }
}

impl core::fmt::Debug for Address<Virtual> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Address<Virtual>")
            .field(&self.as_ptr::<core::ffi::c_void>())
            .finish()
    }
}

impl core::ops::Add<Address<Virtual>> for Address<Virtual> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value.checked_add(rhs.value).unwrap())
    }
}

impl core::ops::Add<usize> for Address<Virtual> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.value + rhs)
    }
}

impl core::ops::AddAssign<usize> for Address<Virtual> {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs
    }
}

impl core::ops::Sub<Address<Virtual>> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value.checked_sub(rhs.value).unwrap())
    }
}

impl core::ops::Sub<usize> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.value - rhs)
    }
}

impl core::ops::SubAssign<usize> for Address<Virtual> {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs
    }
}
