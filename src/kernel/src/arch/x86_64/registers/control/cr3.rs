use crate::task::asid::AddressSpaceId;
use bit_field::BitField;
use core::arch::asm;
use libsys::{
    address::{Address, Frame},
    constants::{page_bits, physical_address_bits},
};

pub struct CR3;

impl CR3 {
    pub unsafe fn write(address: Address<Frame>, address_space_id: AddressSpaceId) {
        let address = address.get().get();
        let address_space_id = usize::from(address_space_id);
        let bits = address | address_space_id;

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) bits
            );
        }
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
