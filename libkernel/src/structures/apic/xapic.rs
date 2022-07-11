use crate::{cell::SyncCell, memory::MMIO};

lazy_static::lazy_static! {
    static ref APIC_MMIO: SyncCell<MMIO> = unsafe {
        SyncCell::new(MMIO::new(crate::registers::msr::IA32_APIC_BASE::get_base_addr().frame_index(), 1))
    };
}

const ICRL: usize = 0x300;
const ICRH: usize = 0x310;

pub struct xAPIC;
impl super::APIC for xAPIC {
    fn read_offset(offset: super::Offset) -> u64 {
        unsafe { APIC_MMIO.read_unchecked((offset as usize) * 0x10) }
    }

    unsafe fn write_offset(offset: super::Offset, value: u64) {
        APIC_MMIO.write_unchecked((offset as usize) * 0x10, value);
    }

    unsafe fn send_int_cmd(int_cmd: super::InterruptCommand) {
        use bit_field::BitField;

        assert!(
            !APIC_MMIO.read_unchecked::<u32>(ICRL as usize).get_bit(12),
            "Cannot send command when command is already pending."
        );

        let raw = int_cmd.get_raw();
        let high = (raw >> 32) as u32;
        let low = raw as u32;

        APIC_MMIO.write_unchecked(ICRH as usize, high);
        APIC_MMIO.write_unchecked(ICRL as usize, low);

        // Wait for pending bit to be cleared.
        while APIC_MMIO.read_unchecked::<u32>(ICRL as usize).get_bit(12) {}
    }
}
