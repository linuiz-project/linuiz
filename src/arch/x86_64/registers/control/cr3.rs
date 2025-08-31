use crate::task::asid::AddressSpaceId;
use bit_field::BitField;
use core::arch::asm;
use libsys::{
    address::{Address, Frame},
    constants::{page_bits, physical_address_bits},
};

pub struct CR3;

impl CR3 {
    pub unsafe fn write(frame: Address<Frame>, address_space_id: AddressSpaceId) {
        let frame_address = frame.get().get();
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

    #[must_use]
    pub fn read() -> (Address<Frame>, AddressSpaceId) {
        let value: usize;

        // Safety: Reading CR3 has no side effects.
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) value,
                options(nostack, nomem, preserves_flags)
            );
        }

        let page_bits = usize::try_from(page_bits().get()).unwrap();
        let physical_address_bits = usize::try_from(physical_address_bits().get()).unwrap();
        let address_space_id_bits = value.get_bits(0..page_bits);
        let frame_index_bits = value.get_bits(page_bits..physical_address_bits);

        let frame = Address::<Frame>::from_index(frame_index_bits)
            .expect("CR3 had non-canonical frame address");
        let address_space_id = AddressSpaceId::new(address_space_id_bits)
            .expect("CR3 had an invalid address space ID");

        (frame, address_space_id)
    }
}
