// mod capabilities;
// pub use capabilities::*;

use crate::mem::io::pci::{Device, Standard};
use libkernel::{LittleEndianU8, LittleEndianU16, LittleEndianU32};

impl Device<Standard> {
    pub fn cardbus_cis_ptr(&self) -> Option<usize> {
        match unsafe { self.read_offset::<LittleEndianU32>(Self::ROW_SIZE * 0xA) } {
            0x0 => None,
            value => Some(value as usize),
        }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.read_offset::<LittleEndianU16>(Self::ROW_SIZE * 0xB) }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { self.read_offset::<LittleEndianU16>((Self::ROW_SIZE * 0xB) + 2) }
    }

    pub fn expansion_rom_base_addr(&self) -> Option<usize> {
        match unsafe { self.read_offset::<LittleEndianU32>(Self::ROW_SIZE * 0xC) } {
            0x0 => None,
            value => Some(value as usize),
        }
    }

    // pub(self) fn capabilities(&self) -> CapablitiesIterator {
    //     CapablitiesIterator::new(&self.mmio, unsafe { (self.mmio.read::<u8>(0x34).assume_init() & !0b11) as usize })
    // }

    // pub fn get_capability<T: capabilities::Capability>(&self) -> Option<T> {
    //     let initial_capability_offset = unsafe { self.read_offset::<LittleEndianU8>(Self::ROW_SIZE * 0xD) };
    //     let capabilities_iterator = CapablitiesIterator::new(self);

    //     for (capability_type, capability_base_ptr) in capabilities_iterator {
    //         if capability_type == T::TYPE_CODE {
    //             return Some(unsafe {
    //                 T::from_base_ptr(
    //                     capability_base_ptr,
    //                     [
    //                         self.get_bar(0),
    //                         self.get_bar(1),
    //                         self.get_bar(2),
    //                         self.get_bar(3),
    //                         self.get_bar(4),
    //                         self.get_bar(5),
    //                     ],
    //                 )
    //             });
    //         }
    //     }

    //     None
    // }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.read_offset::<LittleEndianU8>(Self::ROW_SIZE * 0xF) } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.read_offset::<LittleEndianU8>((Self::ROW_SIZE * 0xF) + 1) } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.read_offset::<LittleEndianU8>((Self::ROW_SIZE * 0xF) + 2) }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.read_offset::<LittleEndianU8>((Self::ROW_SIZE * 0xF) + 3) }
    }
}

impl core::fmt::Debug for Device<Standard> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debug_fmt(debug_struct);
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
