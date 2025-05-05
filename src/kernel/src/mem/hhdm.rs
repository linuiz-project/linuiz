use libsys::{Address, Frame, Page, Virtual};

pub static HHDM: spin::Once<Hhdm> = spin::Once::new();

pub fn get() -> &'static Hhdm {
    HHDM.get().unwrap()
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(Address<Page>);

impl Hhdm {
    pub const fn new(address: Address<Page>) -> Self {
        Self(address)
    }

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
        self.virt().get().checked_add(frame.get().get()).and_then(Address::new)
    }
}
