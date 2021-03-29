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
struct MCFGEntry {
    base_addr: Address<Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl MCFGEntry {
    pub fn configure_busses(&self) {
        debug!(
            "Configuring busses: {}..{}",
            self.start_pci_bus, self.end_pci_bus
        );
        
        for (index, bus) in crate::io::pci::express::iter_busses_mut()
            .enumerate()
            .skip(self.start_pci_bus as usize)
            .take((self.end_pci_bus - self.start_pci_bus) as usize)
        {
            if !bus.is_valid() {
                info!("Configuring PCIe bus {}/255.", index);
                unsafe {
                    *bus = crate::io::pci::express::PCIeBus::new(
                        self.base_addr + ((index as usize) << 20),
                    );
                }
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
