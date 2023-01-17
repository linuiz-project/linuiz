use crate::{checked_virt_canonical, virt_canonical_shift};

pub struct Virtual;

impl super::Addressable for Virtual {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        checked_virt_canonical(init).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        let shift = Self::InitType::BITS - virt_canonical_shift().get();
        (((init << shift) as isize) << shift) as Self::ReprType
    }
}

impl super::PtrAddressable for Virtual {
    fn from_ptr<T>(ptr: *mut T) -> Self::ReprType {
        ptr.addr()
    }

    fn as_ptr(repr: Self::ReprType) -> *mut u8 {
        repr as *mut u8
    }
}
