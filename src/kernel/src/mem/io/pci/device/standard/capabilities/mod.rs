// mod msix;
// pub use msix::*;

use super::DeviceType;
use crate::num::LittleEndianU32;

pub trait Capability {
    const TYPE_CODE: u8;
    const BARS_USED: [bool; super::Standard::REGISTER_COUNT];

    unsafe fn from_base_ptr(capability_base_ptr: *mut LittleEndianU32, bars: [Option<super::BAR>; 6]) -> Self;
}

pub(super) struct CapablitiesIterator {
    base_config_address: usize,
    next_offset: u8,
}

impl CapablitiesIterator {
    pub(super) fn new(pci_config_base_ptr: *mut LittleEndianU32, initial_offset: u8) -> Self {
        Self { base_config_address: pci_config_base_ptr as usize, next_offset: initial_offset }
    }
}

impl Iterator for CapablitiesIterator {
    type Item = (u8, *mut LittleEndianU32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_offset > 0 {
            unsafe {
                use bit_field::BitField;

                let capability_base_ptr =
                    (self.base_config_address + (self.next_offset as usize)) as *mut LittleEndianU32;
                let capability_reg0 = capability_base_ptr.read_volatile().get();
                self.next_offset = capability_reg0.get_bits(8..16) as u8;

                Some((capability_reg0.get_bits(0..8) as u8, capability_base_ptr))
            }
        } else {
            None
        }
    }
}
