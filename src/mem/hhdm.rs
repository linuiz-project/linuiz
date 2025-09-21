use crate::{
    mem::addr::{
        phys::{FrameAddress, PhysicalAddress},
        virt::{PageAddress, VirtualAddress},
    },
    util::sync::Once,
};
use core::num::NonZero;

pub struct HigherHalfDirectMap(NonZero<usize>);

static HIGHER_HALF_DIRECT_MAP: Once<HigherHalfDirectMap> = Once::new();

impl HigherHalfDirectMap {
    pub fn init(hhdm_request: &limine::request::HhdmRequest) {
        HIGHER_HALF_DIRECT_MAP.call_once(|| {
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
            let base_address = usize::try_from(base_address).unwrap();
            let base_address = NonZero::new(base_address).unwrap();

            Self(base_address)
        });
    }

    fn get_static() -> &'static Self {
        HIGHER_HALF_DIRECT_MAP.get().unwrap()
    }

    /// Convert a frame address to its higher-half direct mapped page
    /// counterpart.
    pub fn frame_to_page<F: FrameAddress, P: PageAddress<Frame = F>>(frame: F) -> P {
        let frame_address: usize = frame.into();
        Self::get_static()
            .0
            .checked_add(frame_address)
            .and_then(|frame_address| P::new(frame_address.get()).ok())
            .expect("higher-half direct map offset overflowed")
    }

    /// Convert a frame address to its higher-half direct mapped page
    /// counterpart.
    pub fn physical_to_virtual(address: PhysicalAddress) -> VirtualAddress {
        Self::get_static()
            .0
            .checked_add(usize::from(address))
            .map(NonZero::get)
            .and_then(|frame_address| VirtualAddress::new(frame_address).ok())
            .expect("higher-half direct map offset overflowed")
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

    /// Returns whether the provided address is a higher-half or lower-half
    /// address.
    pub fn is_address_higher_half(address: VirtualAddress) -> bool {
        usize::from(address) >= Self::get_static().0.get()
    }
}
