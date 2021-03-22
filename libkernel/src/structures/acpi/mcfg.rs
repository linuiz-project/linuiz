use crate::{
    addr_ty::Physical,
    structures::acpi::{
        xsdt::{XSDTEntry, XSDTEntryType},
        Checksum, SDTHeader, SizedACPITable,
    },
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
struct MCFGEntry {
    base_addr: Address<Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl MCFGEntry {
    pub fn configure_busses(&self) {
        for bus_index in self.start_pci_bus..=self.end_pci_bus {
            let mut bus = crate::io::pci::express::get_bus(bus_index);
            if !bus.is_valid() {
                trace!("Configuring PCIe bus {}/255.", bus_index);
                *bus = unsafe {
                    crate::io::pci::express::PCIeBus::new(
                        self.base_addr + ((bus_index as usize) << 20),
                    )
                };
            }
        }
    }
}

impl XSDTEntry<MCFG> {
    pub fn init_pcie(&self) {
        self.checksum_panic();
        self.entries()
            .iter()
            .for_each(|entry| entry.configure_busses());
    }
}

impl SizedACPITable<MCFGHeader, MCFGEntry> for XSDTEntry<MCFG> {}
