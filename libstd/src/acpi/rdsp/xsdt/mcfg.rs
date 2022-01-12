use crate::{
    acpi::{rdsp::xsdt::SubTable, Checksum, SDTHeader, SizedACPITable},
    addr_ty::Physical,
    Address,
};

#[repr(C, packed)]
struct Header {
    sdt_header: SDTHeader,
    reserved: [u8; 8],
}

#[repr(C, packed)]
pub struct Entry {
    base_addr: Address<Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl Entry {
    pub fn base_addr(&self) -> Address<Physical> {
        self.base_addr
    }

    pub fn seg_group_num(&self) -> u16 {
        self.seg_group_num
    }

    pub fn start_pci_bus(&self) -> u8 {
        self.start_pci_bus
    }

    pub fn end_pci_bus(&self) -> u8 {
        self.end_pci_bus
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MCFG Entry")
            .field("Base Address", &self.base_addr())
            .field("Segment Group", &self.seg_group_num())
            .field("Bus Range", &(self.start_pci_bus()..=self.end_pci_bus()))
            .finish()
    }
}

pub struct MCFG {}

impl SubTable for MCFG {
    const SIGNATURE: &'static str = &"MCFG";
}

impl MCFG {
    pub fn iter(&self) -> core::slice::Iter<Entry> {
        self.validate_checksum();
        self.entries().iter()
    }
}

impl SizedACPITable<Header, Entry> for MCFG {}
