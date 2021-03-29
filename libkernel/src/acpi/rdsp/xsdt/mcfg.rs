use crate::{
    acpi::{
        rdsp::xsdt::{XSDTEntry, XSDTEntryType},
        Checksum, SDTHeader, SizedACPITable,
    },
    addr_ty::Physical,
    Address,
};

pub enum MCFG {}
impl XSDTEntryType for MCFG {
    const SIGNATURE: &'static str = &"MCFG";
}

#[repr(C)]
struct MCFGHeader {
    sdt_header: SDTHeader,
    reserved: [u8; 8],
}

#[repr(C)]
#[derive(Debug)]
pub struct MCFGEntry {
    base_addr: Address<Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl MCFGEntry {
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

impl XSDTEntry<MCFG> {
    pub fn iter(&self) -> core::slice::Iter<MCFGEntry> {
        self.checksum_panic();
        self.entries().iter()
    }
}

impl SizedACPITable<MCFGHeader, MCFGEntry> for XSDTEntry<MCFG> {}
