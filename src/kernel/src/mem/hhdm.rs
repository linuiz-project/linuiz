use libsys::{Address, Frame, Page, Virtual};

pub static HHDM: spin::Lazy<Hhdm> = spin::Lazy::new(|| {
    #[limine::limine_tag]
    static LIMINE_HHDM: limine::HhdmRequest = limine::HhdmRequest::new(crate::boot::LIMINE_REV);

    let hhdm_address = LIMINE_HHDM
        .get_response()
        .expect("bootloader provided no higher-half direct mapping")
        .offset()
        .try_into()
        .unwrap();

    debug!("HHDM address: {:X?}", hhdm_address);

    Hhdm(Address::<Page>::new(hhdm_address).unwrap())
});

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(Address<Page>);

impl Hhdm {
    #[inline]
    pub const fn page(self) -> Address<Page> {
        self.0
    }

    #[inline]
    pub fn address(self) -> Address<Virtual> {
        self.0.get()
    }

    #[inline]
    pub fn ptr(self) -> *mut u8 {
        self.address().as_ptr()
    }

    #[inline]
    pub fn offset(self, frame: Address<Frame>) -> Option<Address<Page>> {
        self.address().get().checked_add(frame.get().get()).and_then(Address::new)
    }
}
