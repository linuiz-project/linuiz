use crate::{checked_phys_canonical, PAGE_MASK, PAGE_SHIFT, PHYS_NON_CANONICAL_MASK};

pub struct Frame;

impl super::Addressable for Frame {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        (((init & PAGE_MASK) == 0) && checked_phys_canonical(init)).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        init & !PHYS_NON_CANONICAL_MASK & !PAGE_MASK
    }
}

impl super::IndexAddressable for Frame {
    fn from_index(index: usize) -> Option<Self::ReprType> {
        (index <= !PHYS_NON_CANONICAL_MASK).then_some(index << PAGE_SHIFT.get())
    }

    fn index(repr: Self::ReprType) -> usize {
        repr >> PAGE_SHIFT.get()
    }
}
