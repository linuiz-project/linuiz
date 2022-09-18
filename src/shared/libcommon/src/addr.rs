use core::marker::PhantomData;

pub const VADDR_HW_MAX: usize = 0x1000000000000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Physical;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Virtual;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page;

pub trait AddressType {}
impl AddressType for Physical {}
impl AddressType for Virtual {}
impl AddressType for Frame {}
impl AddressType for Page {}
pub trait RawAddressType {}
impl RawAddressType for Physical {}
impl RawAddressType for Virtual {}
impl RawAddressType for Frame {}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address<T: AddressType>(u64, PhantomData<T>);

impl<T: AddressType> Address<T> {
    pub const unsafe fn new_unchecked(address: u64) -> Self {
        Self(address, PhantomData)
    }
}

impl<T: AddressType + RawAddressType> Address<T> {
    pub const fn zero() -> Self {
        Self(0, PhantomData)
    }

    #[inline(always)]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub fn is_aligned_to(self, alignment: u64) -> bool {
        debug_assert!(alignment.is_power_of_two());
        (self.0 & (alignment - 1)) == 0
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
    pub const fn frame_containing(self) -> Address<Frame> {
        Address::<Frame>::new_truncate(self.0)
    }

    #[inline(always)]
    pub const fn is_frame_aligned(self) -> bool {
        (self.0 & 0xFFF) == 0
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
    pub const unsafe fn as_ptr<T>(self) -> *const T {
        self.0 as usize as *const T
    }

    #[inline(always)]
    pub const unsafe fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as usize as *mut T
    }

    #[inline(always)]
    pub const fn get_page_index(self, depth: usize) -> Option<usize> {
        (self.0 as usize).checked_shr((((depth as u32) - 1) * 9) >> 12)
    }
}

impl Address<Frame> {
    #[inline]
    pub const fn new(address: u64) -> Option<Self> {
        if (address & 0xFFF) == 0 && Address::<Physical>::is_canonical(address) {
            Some(Self(address, PhantomData))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn new_truncate(address: u64) -> Self {
        Self(address & !0xFFF, PhantomData)
    }

    #[inline(always)]
    pub const fn index(self) -> usize {
        (self.0 / 0x1000) as usize
    }
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAlign {
    DontCare = 1,
    Align4KiB = 1 << 12,
    Align2MiB = 1 << 9 << 12,
    Align1GiB = 1 << 9 << 9 << 12,
}

impl PageAlign {
    const fn index(self) -> u64 {
        match self {
            PageAlign::DontCare => 0,
            PageAlign::Align4KiB => 1,
            PageAlign::Align2MiB => 2,
            PageAlign::Align1GiB => 3,
        }
    }

    #[inline(always)]
    pub const fn mask(self) -> u64 {
        (self as u64) - 1
    }
}

impl<I: Into<u64>> From<I> for PageAlign {
    #[inline]
    fn from(into: I) -> Self {
        match into.into() {
            1 => Self::Align4KiB,
            2 => Self::Align2MiB,
            3 => Self::Align1GiB,
            _ => Self::DontCare,
        }
    }
}

impl Address<Page> {
    #[inline]
    pub fn new(address: u64, align: PageAlign) -> Option<Self> {
        Address::<Virtual>::new(address).and_then(|address| {
            if address.is_aligned_to(align as u64) {
                Some(Self(address.as_u64() | align.index(), PhantomData))
            } else {
                None
            }
        })
    }

    #[inline(always)]
    pub const fn containing(address: Address<Virtual>, align: PageAlign) -> Self {
        Self((address.as_u64() & !align.mask()) | align.index(), PhantomData)
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T, align: PageAlign) -> Option<Self> {
        if ptr.is_aligned_to(align as usize) {
            Some(Self((ptr.addr() as u64) | align.index(), PhantomData))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn address(self) -> Address<Virtual> {
        // SAFETY: Our constructor already requires the address bits valid.
        unsafe { Address::<Virtual>::new_unchecked(self.0 & !0xF) }
    }

    #[inline(always)]
    pub fn align(self) -> PageAlign {
        PageAlign::from(self.0 & 0xF)
    }

    #[inline(always)]
    pub fn index(self) -> usize {
        self.address().as_usize() / (self.align() as usize)
    }

    #[inline(always)]
    pub fn depth(self) -> Option<usize> {
        match self.align() {
            PageAlign::DontCare => None,
            PageAlign::Align4KiB => Some(1),
            PageAlign::Align2MiB => Some(2),
            PageAlign::Align1GiB => Some(3),
        }
    }

    #[inline]
    pub fn forward_checked(self, count: usize) -> Option<Self> {
        self.address()
            .as_u64()
            .checked_add((count as u64) * (self.align() as u64))
            .and_then(|address| Self::new(address, self.align()))
    }

    #[inline]
    pub fn backward_checked(self, count: usize) -> Option<Self> {
        self.address()
            .as_u64()
            .checked_sub((count as u64) * (self.align() as u64))
            .and_then(|address| Self::new(address, self.align()))
    }

    #[inline]
    pub unsafe fn zero_memory(self) {
        core::ptr::write_bytes(self.address().as_mut_ptr::<u8>(), 0, self.align() as usize);
    }
}

impl core::iter::Step for Address<Page> {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(end.index() - start.index())
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.forward_checked(count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.backward_checked(count)
    }
}

pub struct PageIterator {
    start: Address<Page>,
    current: Address<Page>,
    end: Address<Page>,
}

impl PageIterator {
    pub fn new(start: Address<Page>, count: usize) -> Option<Self> {
        start.forward_checked(count).map(|end| Self { start, current: start, end })
    }

    pub fn start(&self) -> Address<Page> {
        self.start
    }

    pub fn current(&self) -> Address<Page> {
        self.current
    }

    pub fn end(&self) -> Address<Page> {
        self.end
    }

    pub fn reset(&mut self) {
        self.current = self.start();
    }
}

impl Iterator for PageIterator {
    type Item = Address<Page>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.index() < self.end.index()
            && let Some(next_page) = self.current.forward_checked(1)
        {
            let current_page = self.current;
            self.current = next_page;
            Some(current_page)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for PageIterator {
    fn len(&self) -> usize {
        self.end().index() - self.start().index()
    }
}
