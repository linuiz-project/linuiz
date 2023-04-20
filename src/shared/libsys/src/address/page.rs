use crate::{page_mask, page_shift, virt_canonical_mask, Address, Virtual};

pub struct Page;

impl super::Addressable for Page {
    type Init = usize;
    type Repr = usize;
    type Get = Address<Virtual>;

    fn new(init: Self::Init) -> Option<Self::Repr> {
        (((init & page_mask()) == 0) && crate::checked_virt_canonical(init)).then_some(init)
    }

    fn new_truncate(init: Self::Init) -> Self::Repr {
        init & !page_mask()
    }

    fn get(repr: Self::Repr) -> Self::Get {
        Address::new_truncate(repr)
    }
}

impl super::PtrAddressable for Page {
    fn from_ptr<T>(ptr: *mut T) -> Self::Repr {
        ptr.addr()
    }

    fn as_ptr(repr: Self::Repr) -> *mut u8 {
        repr as *mut u8
    }
}

impl super::IndexAddressable for Page {
    fn from_index(index: usize) -> Option<Self::Repr> {
        let non_canonical_bits = !(virt_canonical_mask() >> page_shift().get());
        ((index & non_canonical_bits) == 0).then_some(index << page_shift().get())
    }

    fn index(repr: Self::Repr) -> usize {
        repr >> page_shift().get()
    }
}
