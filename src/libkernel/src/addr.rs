use core::marker::PhantomData;

pub const VADDR_HW_MAX: usize = 0x1000000000000;

pub trait AddressType {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Physical {}
impl AddressType for Physical {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Virtual {}
impl AddressType for Virtual {}

// TODO use `u64` for the internal integer type
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address<T: AddressType>(u64, PhantomData<T>);

impl<T: AddressType> Address<T> {
    pub const fn zero() -> Self {
        Self(0, PhantomData)
    }

    pub const unsafe fn new_unsafe(address: u64) -> Self {
        Self(address, PhantomData)
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub const fn is_aligned_to(&self, alignment: core::num::NonZeroUsize) -> bool {
        (self.0 & ((alignment.get() as u64) - 1)) == 0
    }
}

impl Address<Physical> {
    #[inline(always)]
    pub const fn is_canonical(address: u64) -> bool {
        (address & 0xFFF00000_00000000) == 0
    }

    /// Constructs a new `Address<Physical>` if the provided address is canonical.
    #[inline(always)]
    pub const fn new(address: u64) -> Option<Self> {
        if Self::is_canonical(address) {
            Some(Self(address, PhantomData))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn new_truncate(address: u64) -> Self {
        Self(address & 0xFFFFFFFFFFFFF, PhantomData)
    }

    #[inline(always)]
    pub const fn frame_index(&self) -> usize {
        (self.as_usize() / 0x1000) as usize
    }

    #[inline(always)]
    pub const fn is_frame_aligned(&self) -> bool {
        (self.0 & 0xFFF) == 0
    }
}

impl core::ops::Add<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new_truncate(self.0 + rhs.0)
    }
}

impl core::ops::Add<u64> for Address<Physical> {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new_truncate(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u64> for Address<Physical> {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::Sub<Address<Physical>> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new_truncate(self.0 - rhs.0)
    }
}

impl core::ops::Sub<u64> for Address<Physical> {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        Self::new_truncate(self.0 - rhs)
    }
}

impl core::ops::SubAssign<u64> for Address<Physical> {
    fn sub_assign(&mut self, rhs: u64) {
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
    pub const fn new(address: u64) -> Option<Self> {
        match address >> 47 {
            0 | 0x1FFFF => Some(Self(address, PhantomData)),
            1 => Some(Self::new_truncate(address)),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn new_truncate(address: u64) -> Self {
        Self((((address << 16) as i64) >> 16) as u64, PhantomData)
    }

    #[inline(always)]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new_truncate(ptr as usize as u64)
    }

    #[inline(always)]
    pub const fn page_index(&self) -> usize {
        (self.as_usize() / 0x1000) as usize
    }

    #[inline(always)]
    pub const unsafe fn as_ptr<T>(&self) -> *const T {
        self.0 as usize as *const T
    }

    #[inline(always)]
    pub const unsafe fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.0 as usize as *mut T
    }

    #[inline(always)]
    pub const fn page_offset(&self) -> usize {
        (self.0 & 0xFFF) as usize
    }

    #[inline(always)]
    pub const fn p1_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    #[inline(always)]
    pub const fn p2_index(&self) -> usize {
        ((self.0 >> 12 >> 9) & 0x1FF) as usize
    }

    #[inline(always)]
    pub const fn p3_index(&self) -> usize {
        ((self.0 >> 12 >> 9 >> 9) & 0x1FF) as usize
    }

    #[inline(always)]
    pub const fn p4_index(&self) -> usize {
        ((self.0 >> 12 >> 9 >> 9 >> 9) & 0x1FF) as usize
    }

    #[inline(always)]
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

impl core::ops::Add<u64> for Address<Virtual> {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new_truncate(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u64> for Address<Virtual> {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::Sub<Address<Virtual>> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new_truncate(self.0 - rhs.0)
    }
}

impl core::ops::Sub<u64> for Address<Virtual> {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        Self::new_truncate(self.0 - rhs)
    }
}

impl core::ops::SubAssign<u64> for Address<Virtual> {
    fn sub_assign(&mut self, rhs: u64) {
        self.0 -= rhs;
    }
}
