use crate::{
    addr_ty::Physical,
    io::pci::express::PCIeDevice,
    structures::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable},
    Address,
};
use alloc::{collections::BTreeMap, vec::Vec};

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
    base_addr: Address<Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

// TODO this solution doesn't really feel very permanent, so I'd like to use a more
//      idiomatic approach.
static mut MCFG_ENTRY_BUSSES: BTreeMap<Address<Physical>, Vec<PCIeDevice>> = BTreeMap::new();

fn get_mcfg_entry_busses_vec<'a>(base_addr: Address<Physical>) -> Option<&'a Vec<PCIeDevice>> {
    unsafe { MCFG_ENTRY_BUSSES.get(&base_addr) }
}

impl MCFGEntry {
    pub fn iter(&self) -> core::slice::Iter<PCIeDevice> {
        if get_mcfg_entry_busses_vec(self.base_addr).is_none() {
            debug!("No PCI busses entry found for MCFG entry; creating.");

            let busses = (self.start_pci_bus..self.end_pci_bus)
                .flat_map(|bus_index| (0..32).map(move |device_index| (bus_index, device_index)))
                .filter_map(|(bus_index, device_index)| {
                    let offset_addr =
                        self.base_addr + (((bus_index as usize) << 20) | (device_index << 15));
                    let header = unsafe {
                        &*crate::memory::malloc::get()
                            .physical_memory(offset_addr)
                            .as_ptr::<crate::io::pci::PCIDeviceHeader>()
                    };

                    if !header.is_invalid() {
                        debug!("Non-invalid header: {:?}", header);

                        let mmio_frames = unsafe {
                            crate::memory::falloc::get()
                                .acquire_frame(
                                    offset_addr.frame_index(),
                                    crate::memory::falloc::FrameState::MMIO,
                                )
                                .unwrap()
                                .into_iter()
                        };

                        Some(PCIeDevice::new(
                            crate::memory::mmio::unmapped_mmio(mmio_frames)
                                .unwrap()
                                .map(),
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            unsafe { MCFG_ENTRY_BUSSES.insert(self.base_addr, busses) };
        }

        get_mcfg_entry_busses_vec(self.base_addr).unwrap().iter()
    }
}
