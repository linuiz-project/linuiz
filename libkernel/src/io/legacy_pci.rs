use crate::io::port::ParallelPort;
use bit_field::BitField;
use spin::Mutex;

static LEGACY_PCI_PORTS: Mutex<ParallelPort<u32>> =
    Mutex::new(unsafe { ParallelPort::new(0xCF8, 0xCFC) });

/// Reads a the configuration for a given PCI address.
///
/// Returns a `Some(u16, u16)`, with the first value being the vendor, and
/// the second value being the device. Alternatively, returns `None` when
/// the vendor is 0xFFFF, i.e. a non-existent vendor (and therefore, a
/// non-existent device).
pub fn config_read(config_addr: ConfigAddressPacket) -> Option<(u16, u16)> {
    let mut pci = LEGACY_PCI_PORTS.lock();

    pci.write(config_addr.raw());
    let port_value = pci.read();

    let vendor = port_value as u16;
    let device = (port_value >> 16) as u16;

    if vendor < u16::MAX {
        Some((vendor, device))
    } else {
        None
    }
}

#[repr(transparent)]
pub struct ConfigAddressPacket(u32);

impl ConfigAddressPacket {
    pub fn new(bus: u8, slot: u8, func: u8, offset: u8) -> Self {
        debug_assert_eq!(offset & 0x3, 0, "offset must be 32-bit aligned");

        let mut addr_packet: u32 = 0x80000000;
        addr_packet.set_bits(0..8, offset as u32);
        addr_packet.set_bits(8..11, func as u32);
        addr_packet.set_bits(11..16, slot as u32);
        addr_packet.set_bits(16..24, bus as u32);

        Self { 0: addr_packet }
    }

    pub fn bus(&self) -> u8 {
        self.0.get_bits(16..24) as u8
    }

    pub fn slot(&self) -> u8 {
        self.0.get_bits(11..16) as u8
    }

    pub fn func(&self) -> u8 {
        self.0.get_bits(8..11) as u8
    }

    pub fn offset(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }

    fn raw(&self) -> u32 {
        self.0
    }
}
