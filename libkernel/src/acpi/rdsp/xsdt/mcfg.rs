use crate::{
    acpi::{
        rdsp::xsdt::{XSDTSubTable, XSDTSubTableType},
        Checksum, SDTHeader, SizedACPITable,
    },
    addr_ty::Physical,
    Address,
};

pub enum MCFG {}
impl XSDTSubTableType for MCFG {
    const SIGNATURE: &'static str = &"MCFG";
}

#[repr(C)]
struct MCFGHeader {
    sdt_header: SDTHeader,
    reserved: [u8; 8],
}

#[repr(C)]
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

impl core::fmt::Debug for MCFGEntry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MCFG Entry")
            .field("Base Address", &self.base_addr())
            .field("Segment Group", &self.seg_group_num())
            .field("Bus Range", &(self.start_pci_bus()..=self.end_pci_bus()))
            .finish()
    }
}

impl XSDTSubTable<MCFG> {
    pub fn iter(&self) -> core::slice::Iter<MCFGEntry> {
        self.checksum_panic();
        self.entries().iter()
    }
}

impl SizedACPITable<MCFGHeader, MCFGEntry> for XSDTSubTable<MCFG> {}
