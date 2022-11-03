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

impl<T: AddressType + RawAddressType> Address<T> {
    pub const fn zero() -> Self {
        Self(0, PhantomData)
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn is_aligned_to(self, alignment: u64) -> bool {
        debug_assert!(alignment.is_power_of_two());
        (self.0 & (alignment - 1)) == 0
    }
}

impl Address<Physical> {
    #[inline]
    pub const fn is_canonical(address: u64) -> bool {
        (address & 0xFFF00000_00000000) == 0
    }

    /// Constructs a new `Address<Physical>` if the provided address is canonical.
    #[inline]
    pub const fn new(address: u64) -> Option<Self> {
        if Self::is_canonical(address) {
            Some(Self(address, PhantomData))
        } else {
            None
        }
    }

    #[inline]
    pub const fn new_truncate(address: u64) -> Self {
        Self(address & 0xFFFFFFFFFFFFF, PhantomData)
    }

    #[inline]
    pub const fn frame(self) -> Address<Frame> {
        Address::<Frame>::new_truncate(self)
    }

    #[inline]
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

    #[inline]
    pub const fn new_truncate(address: u64) -> Self {
        Self((((address << 16) as i64) >> 16) as u64, PhantomData)
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new_truncate(ptr as usize as u64)
    }

    #[inline]
    pub const unsafe fn as_ptr<T>(self) -> *const T {
        self.0 as usize as *const T
    }

    #[inline]
    pub const unsafe fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as usize as *mut T
    }

    #[inline]
    pub const fn get_page_index(self, depth: usize) -> Option<usize> {
        (self.0 as usize).checked_shr((((depth as u32) - 1) * 9) >> 12)
    }
}

impl Address<Frame> {
    #[inline]
    pub const fn new(address: Address<Physical>) -> Option<Self> {
        if address.is_frame_aligned() {
            Some(Self(address.as_u64(), PhantomData))
        } else {
            None
        }
    }

    #[inline]
    pub const fn new_truncate(address: Address<Physical>) -> Self {
        Self(address.as_u64() & !0xFFF, PhantomData)
    }

    #[inline]
    pub fn from_u64(address: u64) -> Option<Self> {
        Address::<Physical>::new(address).and_then(Self::new)
    }

    #[inline]
    pub const fn from_u64_truncate(address: u64) -> Self {
        Self::new_truncate(Address::<Physical>::new_truncate(address))
    }

    #[inline]
    pub const fn index(self) -> usize {
        (self.0 / 0x1000) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAlign {
    Align4KiB,
    Align2MiB,
    Align1GiB,
}

impl PageAlign {
    #[inline]
    pub const fn from_u64(value: u64) -> Option<Self> {
        if value == Self::Align4KiB.as_u64() {
            Some(Self::Align4KiB)
        } else if value == Self::Align2MiB.as_u64() {
            Some(Self::Align2MiB)
        } else if value == Self::Align1GiB.as_u64() {
            Some(Self::Align1GiB)
        } else {
            None
        }
    }

    #[inline]
    pub const fn from_usize(value: usize) -> Option<Self> {
        if value == Self::Align4KiB.as_usize() {
            Some(Self::Align4KiB)
        } else if value == Self::Align2MiB.as_usize() {
            Some(Self::Align2MiB)
        } else if value == Self::Align1GiB.as_usize() {
            Some(Self::Align1GiB)
        } else {
            None
        }
    }

    #[inline]
    pub const fn depth(self) -> u64 {
        match self {
            PageAlign::Align4KiB => 1,
            PageAlign::Align2MiB => 2,
            PageAlign::Align1GiB => 3,
        }
    }

    #[inline]
    pub const fn as_u64(self) -> u64 {
        match self {
            PageAlign::Align4KiB => 1 << 12,
            PageAlign::Align2MiB => 1 << 21,
            PageAlign::Align1GiB => 1 << 30,
        }
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.as_u64() as usize
    }

    #[inline]
    pub const fn mask(self) -> u64 {
        self.as_u64() - 1
    }
}

impl Address<Page> {
    const ALIGN_BIT_MASK: u64 = 0b11;

    #[inline]
    pub const fn new(address: Address<Virtual>, align: Option<PageAlign>) -> Option<Self> {
        match align {
            Some(align) if address.is_aligned_to(align.as_u64()) => {
                Some(Self((address.as_u64() & !align.mask()) | align.depth(), PhantomData))
            }

            None => Some(Self(address.as_u64() & !Self::ALIGN_BIT_MASK, PhantomData)),

            Some(_) => None,
        }
    }

    #[inline]
    pub const fn new_truncate(address: Address<Virtual>, align: Option<PageAlign>) -> Self {
        Self(
            (address.as_u64() & !align.map_or(Self::ALIGN_BIT_MASK, PageAlign::mask))
                | align.map_or(0, PageAlign::depth),
            PhantomData,
        )
    }

    #[inline]
    pub fn from_u64(address: u64, align: Option<PageAlign>) -> Option<Self> {
        Address::<Virtual>::new(address).and_then(|address| Self::new(address, align))
    }

    #[inline]
    pub const fn from_u64_truncate(address: u64, align: Option<PageAlign>) -> Self {
        Self::new_truncate(Address::<Virtual>::new_truncate(address), align)
    }

    #[inline]
    pub fn from_ptr<T>(ptr: *const T, align: Option<PageAlign>) -> Option<Self> {
        match align {
            Some(align) if ptr.is_aligned_to(align.as_usize()) => {
                Some(Self(((ptr.addr() as u64) & !align.mask()) | align.depth(), PhantomData))
            }

            None => Some(Self((ptr.addr() as u64) & !Self::ALIGN_BIT_MASK, PhantomData)),

            Some(_) => None,
        }
    }

    #[inline]
    pub const fn address(self) -> Address<Virtual> {
        Address::<Virtual>(self.0 & !Self::ALIGN_BIT_MASK, PhantomData)
    }

    #[inline]
    pub const fn align(self) -> Option<PageAlign> {
        match self.0 & Self::ALIGN_BIT_MASK {
            0 => None,
            1 => Some(PageAlign::Align4KiB),
            2 => Some(PageAlign::Align2MiB),
            3 => Some(PageAlign::Align1GiB),
            _ => unimplemented!(),
        }
    }

    #[inline]
    pub fn index(self) -> Option<usize> {
        self.align().map(|align| self.address().as_usize() / align.as_usize())
    }

    #[inline]
    pub fn depth(self) -> Option<usize> {
        self.align().map(|align| match align {
            PageAlign::Align4KiB => 1,
            PageAlign::Align2MiB => 2,
            PageAlign::Align1GiB => 3,
        })
    }

    #[inline]
    pub const fn is_null(self) -> bool {
        self.address().is_null()
    }

    pub fn forward_checked(self, count: usize) -> Option<Self> {
        self.align().and_then(|align| {
            self.address()
                .as_u64()
                .checked_add((count as u64) * align.as_u64())
                .and_then(|address| Address::<Virtual>::new(address))
                .map(|address| Self::new_truncate(address, Some(align)))
        })
    }

    pub fn backward_checked(self, count: usize) -> Option<Self> {
        self.align().and_then(|align| {
            self.address()
                .as_u64()
                .checked_sub((count as u64) * align.as_u64())
                .and_then(|address| Address::<Virtual>::new(address))
                .map(|address| Self::new_truncate(address, Some(align)))
        })
    }

    #[inline]
    pub unsafe fn zero_memory(self) {
        if let Some(align) = self.align() {
            core::ptr::write_bytes(self.address().as_mut_ptr::<u8>(), 0, align.as_usize());
        } else {
            unimplemented!()
        }
    }
}

impl core::iter::Step for Address<Page> {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        start.index().zip(end.index()).map(|(start, end)| end - start)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.forward_checked(count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.backward_checked(count)
    }
}

// pub struct PageIterator {
//     start: Address<Page>,
//     current: Address<Page>,
//     end: Address<Page>,
// }

// impl PageIterator {
//     pub fn new(start: Address<Page>, count: usize) -> Option<Self> {
//         start.forward_checked(count).map(|end| Self { start, current: start, end })
//     }

//     pub fn start(&self) -> Address<Page> {
//         self.start
//     }

//     pub fn current(&self) -> Address<Page> {
//         self.current
//     }

//     pub fn end(&self) -> Address<Page> {
//         self.end
//     }

//     pub fn reset(&mut self) {
//         self.current = self.start();
//     }
// }

// impl Iterator for PageIterator {
//     type Item = Address<Page>;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.current.index() < self.end.index()
//             && let Some(next_page) = self.current.forward_checked(1)
//         {
//             let current_page = self.current;
//             self.current = next_page;
//             Some(current_page)
//         } else {
//             None
//         }
//     }
// }

// impl ExactSizeIterator for PageIterator {
//     fn len(&self) -> usize {
//         self.end().index() - self.start().index()
//     }
// }
