use core::marker::PhantomData;

pub const VADDR_HW_MAX: usize = 0x1000000000000;

pub trait AddressType {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Physical {}
impl AddressType for Physical {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Virtual {}
impl AddressType for Virtual {}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address<T: AddressType>(usize, PhantomData<T>);

impl<T: AddressType> Address<T> {
    pub const fn zero() -> Self {
        Self(0, PhantomData)
    }

    pub const unsafe fn new_unsafe(addr: usize) -> Self {
        Self(addr, PhantomData)
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    #[inline(always)]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_aligned_to(&self, alignment: usize) -> bool {
        assert!(alignment.is_power_of_two(), "alignment must be a power of two");
        (self.0 & (alignment - 1)) == 0
    }
}

impl Address<Physical> {
    pub const fn new(addr: usize) -> Self {
        match addr >> 52 {
            0 => Self(addr, PhantomData),
            _ => panic!("given address is not canonical (bits 52..64 contain data)"),
        }
    }

    #[inline]
    pub const fn new_truncate(addr: usize) -> Self {
        Self(addr & 0xFFFFFFFFFFFFF, PhantomData)
    }

    #[inline]
    pub const fn frame_index(&self) -> usize {
        (self.as_usize() / 0x1000) as usize
    }

    #[inline]
    pub const fn is_canonical(&self) -> bool {
        (self.0 >> 52) == 0
    }

    #[inline]
    pub const fn is_frame_aligned(&self) -> bool {
        (self.0 & 0xFFF) == 0
    }
}

impl core::ops::Add<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0 + rhs.0)
    }
}

impl core::ops::Add<usize> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

impl core::ops::AddAssign<usize> for Address<Physical> {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl core::ops::Sub<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl core::ops::Sub<usize> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new(self.0 - rhs)
    }
}

impl core::ops::SubAssign<usize> for Address<Physical> {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs;
    }
}

impl core::fmt::Debug for Address<Physical> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Address<Physical>").field(&format_args!("{:#X}", self.0)).finish()
    }
}

impl Address<Virtual> {
    /// Returns a safe instance of a virtual address, or `None` if the provided address is non-canonical.
    pub const fn new(addr: usize) -> Option<Self> {
        match addr >> 47 {
            0 | 0x1FFFF => Some(Self(addr, PhantomData)),
            1 => Some(Self::new_truncate(addr)),
            _ => None,
        }
    }

    #[inline]
    pub const fn new_truncate(addr: usize) -> Self {
        Self((((addr << 16) as isize) >> 16) as usize, PhantomData)
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new_truncate(ptr as usize)
    }

    #[inline]
    pub const fn page_index(&self) -> usize {
        (self.as_usize() / 0x1000) as usize
    }

    #[inline]
    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    #[inline]
    pub const fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0 as *mut T
    }

    #[inline]
    pub const fn page_offset(&self) -> usize {
        (self.0 & 0xFFF) as usize
    }

    #[inline]
    pub const fn p1_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    #[inline]
    pub const fn p2_index(&self) -> usize {
        ((self.0 >> 12 >> 9) & 0x1FF) as usize
    }

    #[inline]
    pub const fn p3_index(&self) -> usize {
        ((self.0 >> 12 >> 9 >> 9) & 0x1FF) as usize
    }

    #[inline]
    pub const fn p4_index(&self) -> usize {
        ((self.0 >> 12 >> 9 >> 9 >> 9) & 0x1FF) as usize
    }

    #[inline]
    pub const fn is_page_aligned(&self) -> bool {
        (self.0 & 0xFFF) == 0
    }
}

impl core::fmt::Debug for Address<Virtual> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Address<Virtual>").field(&format_args!("{:#X}", self.0)).finish()
    }
}

impl core::ops::Add<Address<Virtual>> for Address<Virtual> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new_truncate(self.0 + rhs.0)
    }
}

impl core::ops::Add<usize> for Address<Virtual> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self::new_truncate(self.0 + rhs)
    }
}

impl core::ops::AddAssign<usize> for Address<Virtual> {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl core::ops::Sub<Address<Virtual>> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new_truncate(self.0 - rhs.0)
    }
}

impl core::ops::Sub<usize> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self::new_truncate(self.0 - rhs)
    }
}

impl core::ops::SubAssign<usize> for Address<Virtual> {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs;
    }
}
