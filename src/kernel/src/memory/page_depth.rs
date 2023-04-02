#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageDepth(u32);

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
        libsys::page_size().checked_shl(libsys::table_index_shift().get() * self.get()).unwrap()
    }

    #[inline]
    pub const fn next(self) -> Option<Self> {
        self.get().checked_sub(1).map(PageDepth::new)
    }

    #[inline]
    pub fn is_min(self) -> bool {
        self == Self::min()
    }

    #[inline]
    pub fn is_max(self) -> bool {
        self == Self::max()
    }
}
