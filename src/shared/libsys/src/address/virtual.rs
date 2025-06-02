use crate::{checked_virt_canonical, virt_noncanonical_shift};

#[derive(Debug)]
pub struct Virtual;

impl super::Addressable for Virtual {
    type Init = usize;
    type Repr = usize;
    type Get = usize;

    const DEBUG_NAME: &'static str = "Address<Virtual>";

    fn new(init: Self::Init) -> Option<Self::Repr> {
        checked_virt_canonical(init).then_some(init)
    }

    fn new_truncate(init: Self::Init) -> Self::Repr {
        let sign_extension_shift = Self::Init::BITS - virt_noncanonical_shift().get();
        (((init << sign_extension_shift) as isize) >> sign_extension_shift) as Self::Repr
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
