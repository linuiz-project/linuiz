use libsys::{Address, Frame, Page, Virtual};

static HHDM_ADDRESS: spin::Once<Hhdm> = spin::Once::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(Address<Page>);

impl Hhdm {
    pub fn initialize() {
        #[limine::limine_tag]
        static LIMINE_HHDM: limine::HhdmRequest = limine::HhdmRequest::new(crate::boot::LIMINE_REV);

        HHDM_ADDRESS.call_once(|| {
            let hhdm_address = LIMINE_HHDM
                .get_response()
                .expect("bootloader provided no higher-half direct mapping")
                .offset()
                .try_into()
                .unwrap();

            debug!("HHDM address: {:X?}", hhdm_address);

            Self(Address::<Page>::new(hhdm_address).unwrap())
        });
    }

    fn get() -> Self {
        *HHDM_ADDRESS.get().expect("HHDM address is uninitialized")
    }

    #[inline]
    pub fn page() -> Address<Page> {
        Self::get().0
    }

    #[inline]
    pub fn address() -> Address<Virtual> {
        Self::get().0.get()
    }

    #[inline]
    pub fn ptr() -> *mut u8 {
        Self::address().as_ptr()
    }

    #[inline]
    pub fn offset(frame: Address<Frame>) -> Option<Address<Page>> {
        Self::address().get().checked_add(frame.get().get()).and_then(Address::new)
    }
}
