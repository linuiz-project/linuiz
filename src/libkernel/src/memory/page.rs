use core::{arch::x86_64, marker::PhantomData};

use crate::{Address, Virtual};

pub struct Order1;
pub struct Order9;
pub struct Order18;

pub trait PageOrder {
    const ALIGNMENT: u64;
    const ALINGMENT_MASK: u64 = Self::ALIGNMENT - 1;
    const INDEX_ALIGNMENT_MASK: u64 = (Self::ALIGNMENT >> 12) - 1;
}
impl PageOrder for Order1 {
    const ALIGNMENT: u64 = 1 << 12;
}
impl PageOrder for Order9 {
    const ALIGNMENT: u64 = 1 << 9 << 12;
}
impl PageOrder for Order18 {
    const ALIGNMENT: u64 = 1 << 9 << 9 << 12;
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Page<Order: PageOrder>(u64, PhantomData<Order>);

impl<Order: PageOrder> Page<Order> {
    #[inline(always)]
    pub const fn null() -> Self {
        Self(0, PhantomData)
    }

    #[inline(always)]
    pub const fn from_index(index: usize) -> Option<Self> {
        if (index & Order::INDEX_ALIGNMENT_MASK) == 0 {
            Some(Self(index as u64, PhantomData))
        } else {
            None
        }
    }

    /// Constructs a page from the provided address, or `None` if the address is not page-aligned.
    #[inline(always)]
    pub const fn from_address(address: Address<Virtual>) -> Option<Self> {
        if address.is_aligned_to(
            // SAFETY: Value is non-zero.
            unsafe { core::num::NonZeroUsize::new_unchecked(Order::ALINGMENT_MASK) },
        ) {
            Some(Self(address.as_u64() / Order::ALIGNMENT, PhantomData))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn from_address_contains(address: Address<Virtual>) -> Self {
        Self((address.as_u64() as u64 & !Order::ALINGMENT_MASK) / Order::ALIGNMENT, PhantomData)
    }

    pub fn from_ptr<T>(ptr: *const T) -> Option<Self> {
        if ptr.is_aligned_to(Order::ALIGNMENT) {
            Some(Self((ptr as usize as u64) / Order::ALIGNMENT, PhantomData))
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn range(start_index: usize, end_index: usize) -> core::ops::Range<Self> {
        Self::from_index(start_index)..Self::from_index(end_index)
    }

    #[inline(always)]
    pub const fn index(&self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn address(&self) -> Address<Virtual> {
        crate::Address::<Virtual>::new_truncate(self.0 * Order::ALIGNMENT)
    }

    pub const fn forward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_add(count).map(Self::from_index)
    }

    pub const fn backward_checked(&self, count: usize) -> Option<Self> {
        self.index().checked_sub(count).map(Self::from_index)
    }

    /// Clears the 4KiB region from this page's start to its end.
    #[inline(always)]
    pub unsafe fn clear_memory(&self) {
        core::ptr::write_bytes(self.address().as_mut_ptr::<u8>(), 0, Order::ALIGNMENT);
    }
}

impl<Order: PageOrder> PartialOrd for Page<Order> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<Order: PageOrder> Ord for Page<Order> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<Order: PageOrder> core::iter::Step for Page<Order> {
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start.forward_checked(count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start.backward_checked(count)
    }

    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(end.index() - start.index())
    }
}

impl<Order: PageOrder> core::fmt::Debug for Page<Order> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Page").field(&format_args!("{:X}", self.address().as_usize())).finish()
    }
}

pub struct PageIterator<Order: PageOrder> {
    start: Page<Order>,
    current: Page<Order>,
    end: Page<Order>,
}

impl<Order: PageOrder> PageIterator<Order> {
    pub fn new(start: &Page<Order>, end: &Page<Order>) -> Self {
        Self { start: *start, current: *start, end: *end }
    }

    pub fn start(&self) -> &Page<Order> {
        &self.start
    }

    pub fn current(&self) -> &Page<Order> {
        &self.current
    }

    pub fn end(&self) -> &Page<Order> {
        &self.end
    }

    pub fn captured_len(&self) -> usize {
        self.end().index() - self.start().index()
    }

    pub fn reset(&mut self) {
        self.current = *self.start();
    }
}

impl<Order: PageOrder> Iterator for PageIterator<Order> {
    type Item = Page<Order>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 < self.end.0 {
            let page = self.current.clone();
            self.current.0 += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl<Order: PageOrder> ExactSizeIterator for PageIterator<Order> {
    fn len(&self) -> usize {
        self.end().index() - self.start().index()
    }
}
