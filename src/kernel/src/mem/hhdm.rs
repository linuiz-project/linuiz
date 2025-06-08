use core::{num::NonZero, ptr::NonNull};

use libsys::{Address, Frame, Page, Physical, Virtual};

static HHDM: spin::Once<Hhdm> = spin::Once::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hhdm(NonZero<usize>);

impl Hhdm {
    pub fn init(hhdm_request: &limine::request::HhdmRequest) {
        HHDM.call_once(|| {
            // Zero-based memory offset of the start of the HHDM.
            let hhdm_offset = hhdm_request
                .get_response()
                .expect("bootloader did not provide response to higher-half direct map request")
                .offset();

            debug!("HHDM @ {hhdm_offset:#X}");

            Hhdm(NonZero::new(usize::try_from(hhdm_offset).unwrap()).unwrap())
        });
    }

    fn s() -> Self {
        HHDM.get()
            .expect("higher-half direct map has not been initialized")
}

    /// The raw virtual offset of the beginning of the higher-half direct map.
    fn offset() -> NonZero<usize> {
        HHDM.get()
            .expect("higher-half direct map has not been initialized")
            .0
    }

pub fn base_ptr() -> NonNull<u8> {
HH
}

    /// Convert a physical address to its higher-half direct mapped virtual counterpart.
    pub fn physical_to_virtual(physical_address: Address<Physical>) -> Address<Virtual> {
        Address::new_truncate(Self::offset().get() + physical_address.get())
    }

    /// Convert a virtual address to its physical counterpart.
    ///
    /// # Panics
    ///
    /// If `virtual_address` is not a higher-half direct mapped address.
    pub fn virtual_to_physical(virtual_address: Address<Virtual>) -> Address<Physical> {
        Address::new(virtual_address.get() - Self::offset().get()).unwrap()
    }

    /// Convert a frame address to its higher-half direct mapped page counterpart.
    pub fn frame_to_page(frame_address: Address<Frame>) -> Address<Page> {
        Address::new_truncate(Self::offset().get() + frame_address.get().get())
    }

    /// Convert a page address to its physical counterpart.
    ///
    /// # Panics
    ///
    /// If `page_address` is not a higher-half direct mapped address.
    pub fn page_to_frame(page_address: Address<Page>) -> Address<Frame> {
        Address::new(page_address.get().get() - Self::offset().get()).unwrap()
    }
}
