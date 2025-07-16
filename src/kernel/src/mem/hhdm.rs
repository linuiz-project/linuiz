use core::num::NonZero;
use libsys::{Address, Frame, Page, Physical, Virtual};

crate::singleton! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub HigherHalfDirectMap {
        base_address: NonZero<usize>,
    }

    fn init(hhdm_request: &limine::request::HhdmRequest) {
        // Zero-based memory offset of the start of the HHDM.
        let base_address = hhdm_request
            .get_response()
            .expect("bootloader did not provide response to higher-half direct map request")
            .offset();

        let base_address = usize::try_from(base_address)
            .ok()
            .and_then(NonZero::new)
            .expect("higher-half direct map offset is invalid");

        debug!("HHDM @ {base_address:#X}");

        Self { base_address }
    }
}

impl HigherHalfDirectMap {
    /// Positively offset `address` by the base address of the higher-half direct map.
    pub fn offset(address: usize) -> NonZero<usize> {
        Self::get_static()
            .base_address
            .get()
            .checked_add(address)
            .and_then(NonZero::new)
            .expect("provided higher-half direct map offset caused overflow")
    }

    /// Negatively offset `address` by the base address of the higher-half direct map.
    pub fn negative_offset(address: usize) -> NonZero<usize> {
        address
            .checked_sub(Self::get_static().base_address.get())
            .and_then(NonZero::new)
            .expect("provided higher-half direct map offset caused underflow")
    }

    /// Convert a physical address to its higher-half direct mapped virtual counterpart.
    pub fn physical_to_virtual(physical_address: Address<Physical>) -> Address<Virtual> {
        Address::new_truncate(Self::get_static().base_address.get() + physical_address.get())
    }

    /// Convert a virtual address to its physical counterpart.
    ///
    /// # Panics
    ///
    /// If `virtual_address` is not a higher-half direct mapped address.
    pub fn virtual_to_physical(virtual_address: Address<Virtual>) -> Address<Physical> {
        Address::new(virtual_address.get() - Self::get_static().base_address.get()).unwrap()
    }

    /// Convert a frame address to its higher-half direct mapped page counterpart.
    pub fn frame_to_page(frame_address: Address<Frame>) -> Address<Page> {
        Address::new_truncate(Self::get_static().base_address.get() + frame_address.get().get())
    }

    /// Convert a page address to its physical counterpart.
    ///
    /// # Panics
    ///
    /// If `page_address` is not a higher-half direct mapped address.
    pub fn page_to_frame(page_address: Address<Page>) -> Address<Frame> {
        Address::new(page_address.get().get() - Self::get_static().base_address.get()).unwrap()
    }
}
