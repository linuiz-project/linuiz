mod msix;
pub use msix::*;

use crate::memory::mmio::{Mapped, MMIO};

/// An exaplanation of the acronyms used here can be inferred from:
///  https://lekensteyn.nl/files/docs/PCI_SPEV_V3_0.pdf table H-1
#[derive(Debug)]
pub enum PCICapablities<'cap> {
    /// PCI Power Management Interface
    PWMI,
    /// Accelerated Graphics Port
    AGP,
    /// Vital Product Data
    VPD,
    /// Slot Identification
    SIDENT,
    /// Message Signaled Interrupts
    MSI,
    /// CompactPCI Hot Swap
    CPCIHS,
    /// PCI-X
    PCIX,
    /// HyperTransport
    HYTPT,
    /// Vendor Specific
    VENDOR,
    /// Debug Port
    DEBUG,
    /// CompactPCI Central Resource Control
    CPCICPC,
    /// PCI Hot-Plug
    HOTPLG,
    /// PCI Bridge Subsystem Vendor ID
    SSYSVENDID,
    /// AGP 8x
    AGP8X,
    /// Secure Device
    SECURE,
    /// PCI Express
    PCIE,
    /// Message Signaled Interrupt Extension
    MSIX(&'cap MSIX),
    Reserved,
    NotImplemented,
}

pub struct PCICapablitiesIterator<'mmio> {
    mmio: &'mmio MMIO<Mapped>,
    offset: u8,
}

impl<'mmio> PCICapablitiesIterator<'mmio> {
    pub(super) fn new(mmio: &'mmio MMIO<Mapped>, offset: u8) -> Self {
        Self { mmio, offset }
    }
}

impl<'mmio> Iterator for PCICapablitiesIterator<'mmio> {
    type Item = PCICapablities<'mmio>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset > 0 {
            unsafe {
                use bit_field::BitField;

                let cap_reg = self.mmio.read::<u32>(self.offset as usize).unwrap();
                let old_offset = self.offset as usize;
                self.offset = cap_reg.get_bits(8..16) as u8;

                Some(match cap_reg.get_bits(0..8) {
                    0x1 => PCICapablities::PWMI,
                    0x2 => PCICapablities::AGP,
                    0x3 => PCICapablities::VPD,
                    0x4 => PCICapablities::SIDENT,
                    0x5 => PCICapablities::MSI,
                    0x6 => PCICapablities::CPCIHS,
                    0x7 => PCICapablities::PCIX,
                    0x8 => PCICapablities::HYTPT,
                    0x9 => PCICapablities::VENDOR,
                    0xA => PCICapablities::DEBUG,
                    0xB => PCICapablities::CPCICPC,
                    0xC => PCICapablities::HOTPLG,
                    0xD => PCICapablities::SSYSVENDID,
                    0xE => PCICapablities::AGP8X,
                    0xF => PCICapablities::SECURE,
                    0x10 => PCICapablities::PCIE,
                    0x11 => PCICapablities::MSIX(self.mmio.borrow(old_offset).unwrap()),
                    0x0 | 0x12..0xFF => PCICapablities::Reserved,
                    _ => PCICapablities::NotImplemented,
                })
            }
        } else {
            None
        }
    }
}
