use core::iter::Step;
use libsys::{
    address::{Address, Virtual},
    constants::{page_bits, table_index_bits, table_index_mask},
};

/// Describes the depth of a page table translation, from min (usually 4) to max
/// (usually 0).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Depth(u32);

impl Depth {
    /// Minimum table depth, typically down to 4KB-sized memory chunks.
    pub fn max() -> Self {
        Self(1)
    }

    /// Minimum table depth, typically down to 2MB-sized memory chunks.
    pub fn large() -> Self {
        Self(2)
    }

    /// Minimum table depth, typically down to 1GB-sized memory chunks.
    pub fn huge() -> Self {
        Self(3)
    }

    /// Minimum table tree depth. On x64, this is 5 levels with LA57 enabled, or
    /// 4 level without.
    pub fn min() -> Self {
        let depth = {
            cfg_select! {
                all(target_arch = "x86_64", test) => { 4 }

                all(target_arch = "x86_64", not(test)) => {
                    use crate::arch::x86_64::registers::control::cr4;

                    if cr4::CR4::read().contains(cr4::Flags::LA57) {
                        5
                    } else {
                        4
                    }
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
        // Because `Depth` is 1-based (i.e. 4-level paging allows us to have a maximum
        // depth of 1), it means we need to adjust the actual depth number to be
        // zero-based for our calcualtion.
        let depth_zero_based = self.get() - 1;
        1usize << page_bits().get() << (table_index_bits().get() * depth_zero_based)
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
        // Because `Depth` is 1-based (i.e. 4-level paging allows us to have a maximum
        // depth of 1), it means we need to adjust the actual depth number to be
        // zero-based for our calcualtion.
        let depth_zero_based = self.get() - 1;
        let index_bit_shift = (depth_zero_based * table_index_bits().get()) + page_bits().get();
        (address.get() >> index_bit_shift) & table_index_mask()
    }
}

impl Step for Depth {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        // We reverse the steps since `Depth` is traversed backwards.
        Step::steps_between(&end.0, &start.0)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let count = u32::try_from(count).ok()?;
        let total = start.0.checked_sub(count)?;

        Self::new(total)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let count = u32::try_from(count).ok()?;
        let total = start.0.checked_add(count)?;

        Self::new(total)
    }
}

#[cfg(test)]
mod tests {
    use super::Depth;
    use libsys::address::{Address, Virtual};

    #[test]
    pub fn new() {
        assert_eq!(Depth::new(0), None);
        assert_eq!(Depth::new(8), None);
        assert_eq!(Depth::new(4), Some(Depth(4)));
    }

    #[test]
    pub fn sizes() {
        assert_eq!(Depth::large(), Depth(2));
        assert_eq!(Depth::large().align(), 0x200000);

        assert_eq!(Depth::huge(), Depth(3));
        assert_eq!(Depth::huge().align(), 0x40000000);
    }

    #[test]
    pub fn step_trait() {
        let mut depth = Depth::min();
        assert_eq!(depth.next_checked(), Some(Depth(3)));
        depth = depth.next();
        assert_eq!(depth.next_checked(), Some(Depth(2)));
        depth = depth.next();
        assert_eq!(depth.next_checked(), Some(Depth(1)));
        depth = depth.next();
        assert_eq!(depth.next_checked(), None);

        depth = Depth::max();
        assert_eq!(depth.next_checked(), None);
    }

    #[test]
    pub fn index_of() {
        // Safety: Virtual address is canonical and not used as an actual address.
        let address = unsafe { Address::<Virtual>::new_unchecked(0xFFFF8000FEE00000) };

        assert_eq!(Depth(4).index_of(address), 256);
        assert_eq!(Depth(3).index_of(address), 3);
        assert_eq!(Depth(2).index_of(address), 503);
        assert_eq!(Depth(1).index_of(address), 0);
    }
}
