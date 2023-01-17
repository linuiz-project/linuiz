use crate::{checked_virt_canonical, virt_noncanonical_mask, PAGE_MASK, PAGE_SHIFT};

pub struct Page;

impl super::Addressable for Page {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        (((init & PAGE_MASK) == 0) && checked_virt_canonical(init)).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        init & !PAGE_MASK
    }
}

impl super::PtrAddressable for Page {
    fn from_ptr<T>(ptr: *mut T) -> Self::ReprType {
        ptr.addr()
    }

    fn as_ptr(repr: Self::ReprType) -> *mut u8 {
        repr as *mut u8
    }
}

impl super::IndexAddressable for Page {
    fn from_index(index: usize) -> Option<Self::ReprType> {
        let noncanonical_bits = !(virt_noncanonical_mask() >> PAGE_SHIFT.get());
        ((index & noncanonical_bits) == 0).then_some(index << PAGE_SHIFT.get())
    }

    fn index(repr: Self::ReprType) -> usize {
        repr >> PAGE_SHIFT.get()
    }
}
