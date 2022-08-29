mod capabilities;
pub use capabilities::*;

use crate::{
    arch::x64::structures::gdt::init,
    memory::io::pci::{Device, DeviceType, Standard, BAR},
};
use bit_field::BitField;

impl Device<Standard> {
    pub fn cardbus_cis_ptr(&self) -> Option<usize> {
        match unsafe { self.base_ptr.add(0xA).read_volatile() }.get() {
            0x0 => None,
            value => Some(value as usize),
        }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.base_ptr.add(0xB).read_volatile() }.get() as u16
    }

    pub fn subsystem_id(&self) -> u16 {
        (unsafe { self.base_ptr.add(0xB).read_volatile() }.get() >> 16) as u16
    }

    pub fn expansion_rom_base_addr(&self) -> Option<usize> {
        match unsafe { self.base_ptr.add(0xC).read_volatile() }.get() {
            0x0 => None,
            value => Some(value as usize),
        }
    }

    // pub(self) fn capabilities(&self) -> CapablitiesIterator {
    //     CapablitiesIterator::new(&self.mmio, unsafe { (self.mmio.read::<u8>(0x34).assume_init() & !0b11) as usize })
    // }

    pub fn get_capability<T: capabilities::Capability>(&self) -> Option<T> {
        let initial_capability_offset = unsafe { self.base_ptr.add(0xD).read_volatile() }.get() as u8;
        let capabilities_iterator = CapablitiesIterator::new(self.base_ptr, initial_capability_offset);

        for (capability_type, capability_base_ptr) in capabilities_iterator {
            if capability_type == T::TYPE_CODE {
                return Some(unsafe {
                    T::from_base_ptr(
                        capability_base_ptr,
                        [
                            self.get_bar(0),
                            self.get_bar(1),
                            self.get_bar(2),
                            self.get_bar(3),
                            self.get_bar(4),
                            self.get_bar(5),
                        ],
                    )
                });
            }
        }

        None
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.base_ptr.add(0xF).read_volatile() }.get().get_bits(0..8) {
            0xFF => None,
            value => Some(value as u8),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.base_ptr.add(0xF).read_volatile() }.get().get_bits(8..16) {
            0x0 => None,
            value => Some(value as u8),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.base_ptr.add(0xF).read_volatile() }.get().get_bits(16..24) as u8
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.base_ptr.read_volatile() }.get().get_bits(24..32) as u8
    }
}

impl core::fmt::Debug for Device<Standard> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debut_fmt(debug_struct);
        debug_struct
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field("Expansion ROM Base Address", &self.expansion_rom_base_addr())
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}
