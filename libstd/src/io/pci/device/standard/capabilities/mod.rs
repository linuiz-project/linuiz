mod msix;
pub use msix::*;

use crate::memory::mmio::{Mapped, MMIO};

/// An exaplanation of the acronyms used here can be inferred from:
///  https://lekensteyn.nl/files/docs/PCI_SPEV_V3_0.pdf table H-1
#[derive(Debug)]
pub enum Capablities<'cap> {
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

pub struct CapablitiesIterator<'mmio> {
    mmio: &'mmio MMIO<Mapped>,
    offset: u8,
}

impl<'mmio> CapablitiesIterator<'mmio> {
    pub(super) fn new(mmio: &'mmio MMIO<Mapped>, offset: u8) -> Self {
        Self { mmio, offset }
    }
}

impl<'mmio> Iterator for CapablitiesIterator<'mmio> {
    type Item = Capablities<'mmio>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset > 0 {
            unsafe {
                use bit_field::BitField;

                let capability_reg_0 = self.mmio.read::<u32>(self.offset as usize).unwrap();
                let old_offset = self.offset as usize;
                self.offset = capability_reg_0.get_bits(8..16) as u8;

                Some(match capability_reg_0.get_bits(0..8) {
                    0x1 => Capablities::PWMI,
                    0x2 => Capablities::AGP,
                    0x3 => Capablities::VPD,
                    0x4 => Capablities::SIDENT,
                    0x5 => Capablities::MSI,
                    0x6 => Capablities::CPCIHS,
                    0x7 => Capablities::PCIX,
                    0x8 => Capablities::HYTPT,
                    0x9 => Capablities::VENDOR,
                    0xA => Capablities::DEBUG,
                    0xB => Capablities::CPCICPC,
                    0xC => Capablities::HOTPLG,
                    0xD => Capablities::SSYSVENDID,
                    0xE => Capablities::AGP8X,
                    0xF => Capablities::SECURE,
                    0x10 => Capablities::PCIE,
                    0x11 => Capablities::MSIX(self.mmio.borrow(old_offset).unwrap()),
                    0x0 | 0x12..0xFF => Capablities::Reserved,
                    _ => Capablities::NotImplemented,
                })
            }
        } else {
            None
        }
    }
}
