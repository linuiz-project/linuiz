use core::iter::Step;
use libsys::{Address, Virtual, page_shift, table_index_mask, table_index_shift};

/// Describes the depth of a page table translation, from min (usually 4) to max (usually 0).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Depth(u32);

impl Depth {
    /// Minimum table depth, typically down to 4KB-sized memory chunks.
    pub fn max() -> Self {
        Self(1)
    }

    /// Minimum table depth, typically down to 2MB-sized memory chunks.
    pub fn mega() -> Self {
        Self(2)
    }

    /// Minimum table depth, typically down to 1GB-sized memory chunks.
    pub fn giga() -> Self {
        Self(3)
    }

    /// Minimum table tree depth. On x64, this is 5 levels with LA57 enabled, or 4 level without.
    pub fn min() -> Self {
        let depth = {
            #[cfg(target_arch = "x86_64")]
            {
                use crate::arch::x86_64::registers::control::{CR4, CR4Flags};

                if CR4::read().contains(CR4Flags::LA57) {
                    5
                } else {
                    4
                }
            }
        };

        Self(depth)
    }

    pub fn max_align() -> usize {
        Self::max().align()
    }

    pub fn min_align() -> usize {
        Self::min().align()
    }

    pub fn new(depth: u32) -> Option<Self> {
        (Self::max().0..=Self::min().0)
            .contains(&depth)
            .then_some(Self(depth))
    }

    pub fn get(self) -> u32 {
        self.0
    }

    pub fn align(self) -> usize {
        libsys::page_size()
            .checked_shl(libsys::table_index_shift().get() * self.get())
            .unwrap()
    }

    pub fn next(self) -> Self {
        Step::forward(self, 1)
    }

    pub fn next_checked(self) -> Option<Self> {
        Step::forward_checked(self, 1)
    }

    pub fn is_max(self) -> bool {
        self == Self::max()
    }

    pub fn is_min(self) -> bool {
        self == Self::min()
    }

    pub fn index_of(self, address: Address<Virtual>) -> usize {
        // Because `Depth` is 1-based (i.e. 4-level paging allows us to have a maximum depth of 1),
        // it means we need to adjust the actual depth number to be zero-based for our calcualtion.
        let base_zero_depth = self.get() - 1;
        let index_bit_shift = (base_zero_depth * table_index_shift().get()) + page_shift().get();
        (address.get() >> index_bit_shift) & table_index_mask()
    }
}

impl Step for Depth {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        Step::steps_between(&end.0, &start.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let count = u32::try_from(count).expect("step count too large");
        let total = start.0.checked_sub(count).expect("step count overflowed");

        Self::new(total)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let count = u32::try_from(count).expect("step count too large");
        let total = start.0.checked_add(count).expect("step count overflowed");

        Self::new(total)
    }
}
