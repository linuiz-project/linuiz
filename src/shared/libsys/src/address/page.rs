use crate::{Address, Virtual, checked_virt_canonical, page_mask, page_shift};

pub struct Page;

impl super::Addressable for Page {
    type Init = usize;
    type Repr = usize;
    type Get = Address<Virtual>;

    const DEBUG_NAME: &'static str = "Address<Page>";

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
        let address = index.rotate_left(page_shift().get());
        ((address & page_mask()) == 0 && checked_virt_canonical(address)).then_some(address)
    }

    fn index(repr: Self::Repr) -> usize {
        repr >> page_shift().get()
    }
}
