use crate::{
    addr_ty::Physical,
    structures::acpi::{Checksum, SDTHeader},
    Address,
};

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

impl Checksum for MCFG {
    fn bytes_len(&self) -> usize {
        self.header().sdt_header().table_len() as usize
    }
}

impl MCFG {
    pub fn header(&self) -> MCFGHeader {
        self.header
    }

    pub fn init_pcie(&self) {
        self.checksum_panic();

        unsafe {
            use core::mem::size_of;

            &*core::ptr::slice_from_raw_parts(
                (self as *const _ as *const u8).add(size_of::<MCFGHeader>()) as *const MCFGEntry,
                ((self.header().sdt_header().table_len() as usize) - size_of::<MCFGHeader>())
                    / size_of::<MCFGEntry>(),
            )
        }
        .iter()
        .for_each(|entry| entry.configure_busses());
    }
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
