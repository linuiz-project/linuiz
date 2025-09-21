use crate::mem::{
    AddressSpaceId,
    addr::phys::{FrameAddress, StandardFrame},
};
use core::arch::asm;

pub struct CR3;

impl CR3 {
    const ADDRESS_SPACE_ID_MASK: usize = 0xFFF;
    const FRAME_ADDRESS_MASK: usize = !0xFFF;

    pub unsafe fn write(frame: StandardFrame, address_space_id: AddressSpaceId) {
        let frame_address = usize::from(frame);
        let address_space_id = usize::from(address_space_id);
        let bits = frame_address | address_space_id;

        trace!("Swapping: {{ id: {address_space_id}, frame: {frame_address:#X} }}");

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) bits
            );
        }

        trace!("Swapped.");
    }

    pub fn read() -> (StandardFrame, AddressSpaceId) {
        let value: usize;

        // Safety: Reading CR3 has no side effects.
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) value,
                options(nostack, nomem, preserves_flags)
            );
        }

        let address_space_id = value & Self::ADDRESS_SPACE_ID_MASK;
        let frame = value & Self::FRAME_ADDRESS_MASK;

        debug_assert!(address_space_id <= AddressSpaceId::MAX);
        debug_assert!(StandardFrame::check_canonical(frame));

        // Safety: Bitwise AND w/ `Self::ADDRESS_SPACE_ID_MASK` ensures value is
        // ≤`AddressSpaceId::MAX`.
        let address_space_id = unsafe { AddressSpaceId::new_unchecked(address_space_id) };
        let frame = StandardFrame::new_truncate(frame);

        (frame, address_space_id)
    }
}
