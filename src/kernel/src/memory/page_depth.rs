#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, Ord)]
pub struct PageDepth(u32);

impl const PartialEq for PageDepth {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl const PartialOrd for PageDepth {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl PageDepth {
    #[inline]
    pub const fn min() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn max() -> Self {
        Self({
            #[cfg(feature = "hugemem")]
            {
                5
            }

            #[cfg(not(feature = "hugemem"))]
            {
                4
            }
        })
    }

    pub fn current() -> Self {
        Self(crate::memory::current_paging_levels())
    }

    #[inline]
    pub const fn min_align() -> usize {
        Self::min().align()
    }

    #[inline]
    pub const fn max_align() -> usize {
        Self::max().align()
    }

    #[inline]
    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn align(self) -> usize {
        libsys::page_size().get().checked_shl(libsys::table_index_shift().get() * self.get()).unwrap()
    }

    #[inline]
    pub const fn next(self) -> Option<Self> {
        self.get().checked_sub(1).map(PageDepth::new)
    }

    #[inline]
    pub const fn is_min(self) -> bool {
        self == Self::min()
    }

    #[inline]
    pub const fn is_max(self) -> bool {
        self == Self::max()
    }
}
