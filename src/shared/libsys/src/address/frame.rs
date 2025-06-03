use crate::{
    Address, Physical, checked_phys_canonical, page_mask, page_shift, phys_canonical_mask,
};

pub struct Frame;

impl super::Addressable for Frame {
    type Init = usize;
    type Repr = usize;
    type Get = Address<Physical>;

    const DEBUG_NAME: &'static str = "Address<Frame>";

    fn new(init: Self::Init) -> Option<Self::Repr> {
        (((init & page_mask()) == 0) && checked_phys_canonical(init)).then_some(init)
    }

    fn new_truncate(init: Self::Init) -> Self::Repr {
        init & phys_canonical_mask() & !page_mask()
    }

    fn get(repr: Self::Repr) -> Self::Get {
        Address::new_truncate(repr)
    }
}

impl super::IndexAddressable for Frame {
    fn from_index(index: usize) -> Option<Self::Repr> {
        (index <= phys_canonical_mask()).then_some(index << page_shift().get())
    }

    fn index(repr: Self::Repr) -> usize {
        repr >> page_shift().get()
    }
}
