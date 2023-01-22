use crate::{checked_virt_canonical, virt_canonical_shift};

pub struct Virtual;

impl super::Addressable for Virtual {
    type Init = usize;
    type Repr = usize;
    type Get = usize;

    fn new(init: Self::Init) -> Option<Self::Repr> {
        checked_virt_canonical(init).then_some(init)
    }

    fn new_truncate(init: Self::Init) -> Self::Repr {
        let shift = Self::Init::BITS - virt_canonical_shift().get();
        (((init << shift) as isize) << shift) as Self::Repr
    }

    fn get(repr: Self::Repr) -> Self::Get {
        repr
    }
}

impl super::PtrAddressable for Virtual {
    fn from_ptr<T>(ptr: *mut T) -> Self::Repr {
        ptr.addr()
    }

    fn as_ptr(repr: Self::Repr) -> *mut u8 {
        repr as *mut u8
    }
}
