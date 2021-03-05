use crate::structures::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};
use x86_64::PhysAddr;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MCFGHeader {
    sdt_header: SDTHeader,
    _reserved: [u8; 8],
}

impl MCFGHeader {
    pub fn sdt_header(&self) -> &SDTHeader {
        &self.sdt_header
    }
}

#[repr(C)]
pub struct MCFG {
    header: MCFGHeader,
}

impl ACPITable for MCFG {
    fn body_len(&self) -> usize {
        (self.header().sdt_header().table_len() as usize) - core::mem::size_of::<MCFGHeader>()
    }
}

impl SizedACPITable<MCFGHeader, MCFGEntry> for MCFG {}

impl Checksum for MCFG {
    fn bytes_len(&self) -> usize {
        self.header().sdt_header().table_len() as usize
    }
}

impl MCFG {
    pub fn header(&self) -> MCFGHeader {
        self.header
    }

    pub fn iter(&self) -> core::slice::Iter<MCFGEntry> {
        self.entries().iter()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct MCFGEntry {
    base_addr: PhysAddr,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl MCFGEntry {
    // pub fn iter_busses(&self) {
    //     for bus_index in self.start_pci_bus..self.end_pci_bus {
    //         use crate::memory::mmio::{unmapped_mmio, MMIO};
    //         let offset_addr = base_addr + (bus_index << 20);
    //         let mmio = unmapped_mmio(offset_addr, 0x1000).unwrap();
    //         mmio.unsafe_map()
    //     }
    // }
}
