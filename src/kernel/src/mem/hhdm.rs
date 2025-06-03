use libsys::{Address, Frame, Page, Virtual};

pub static HHDM: spin::Once<Hhdm> = spin::Once::new();

pub fn set(hhdm_request: &limine::request::HhdmRequest) {
    HHDM.call_once(|| {
        let hhdm_address = hhdm_request
            .get_response()
            .expect("bootloader did not provide HHDM response")
            .offset();

        debug!("HHDM @ {hhdm_address:#X}");

        Hhdm(Address::<Page>::new(hhdm_address.try_into().unwrap()).unwrap())
    });
}

pub fn get() -> &'static Hhdm {
    HHDM.get().unwrap()
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(Address<Page>);

impl Hhdm {
    pub const fn page(self) -> Address<Page> {
        self.0
    }

    pub fn virt(self) -> Address<Virtual> {
        self.0.get()
    }

    pub fn ptr(self) -> *mut u8 {
        self.virt().as_ptr()
    }

    pub fn offset(self, frame: Address<Frame>) -> Option<Address<Page>> {
        self.virt()
            .get()
            .checked_add(frame.get().get())
            .and_then(Address::new)
    }
}
