use core::num::NonZero;
use libsys::address::{Address, Frame, Page, Physical, Virtual};

pub struct HigherHalfDirectMap(NonZero<usize>);

static HIGHER_HALF_DIRECT_MAP: spin::Once<HigherHalfDirectMap> = spin::Once::new();

impl HigherHalfDirectMap {
    pub fn init(hhdm_request: &limine::request::HhdmRequest) -> Self {
        // This function cannot contain any debug logging, as it's used by the
        // local APIC module to offset the register addresses in xAPIC mode,
        // which is then used by the logger to print out the processor ID.
        //
        // So, imagine the case:
        // 1. You log in this function...
        // 2. The logger tries to read the processor ID from the local APIC...
        // 3. The local APIC reads the register address, and tries to read the
        //    higher-half direct map value to offset it...
        // 4. The log function panicks, because the higher-half direct map is not yet
        //    set.

        let base_address = hhdm_request
            .get_response()
            .expect("bootloader did not provide response to higher-half direct map request")
            .offset();

        let base_address = usize::try_from(base_address)
            .ok()
            .and_then(NonZero::new)
            .expect("higher-half direct map offset is invalid");

        Self(base_address)
    }

    fn get_static() -> &'static Self {
        HIGHER_HALF_DIRECT_MAP.get().unwrap()
    }

    /// Positively offset `address` by the base address of the higher-half
    /// direct map.
    pub fn offset(address: usize) -> NonZero<usize> {
        Self::get_static()
            .0
            .get()
            .checked_add(address)
            .and_then(NonZero::new)
            .expect("provided higher-half direct map offset caused overflow")
    }

    /// Negatively offset `address` by the base address of the higher-half
    /// direct map.
    pub fn negative_offset(address: usize) -> NonZero<usize> {
        address
            .checked_sub(Self::get_static().0.get())
            .and_then(NonZero::new)
            .expect("provided higher-half direct map offset caused underflow")
    }

    /// Convert a physical address to its higher-half direct mapped virtual
    /// counterpart.
    pub fn physical_to_virtual(physical_address: Address<Physical>) -> Address<Virtual> {
        Address::<Virtual>::new_truncate(Self::get_static().0.get() + physical_address.get())
    }

    /// Convert a virtual address to its physical counterpart.
    ///
    /// # Panics
    ///
    /// If `virtual_address` is not a higher-half direct mapped address.
    pub fn virtual_to_physical(virtual_address: Address<Virtual>) -> Address<Physical> {
        Address::<Physical>::new(virtual_address.get() - Self::get_static().0.get()).unwrap()
    }

    /// Convert a frame address to its higher-half direct mapped page
    /// counterpart.
    pub fn frame_to_page(frame: Address<Frame>) -> Address<Page> {
        Address::<Page>::new_truncate(Self::get_static().0.get() + frame.get().get())
    }

    /// Returns whether the provided address is a higher-half or lower-half
    /// address.
    pub fn is_address_higher_half(address: Address<Virtual>) -> bool {
        address.get() >= Self::get_static().0.get()
    }
}
