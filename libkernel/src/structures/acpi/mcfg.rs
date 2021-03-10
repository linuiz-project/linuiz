use crate::structures::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};

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
    base_addr: crate::Address<crate::addr_ty::Physical>,
    seg_group_num: u16,
    start_pci_bus: u8,
    end_pci_bus: u8,
    reserved: [u8; 4],
}

impl MCFGEntry {
    #[cfg(feature = "kernel_impls")]
    pub fn iter(&self) {
        for bus_index in self.start_pci_bus..self.end_pci_bus {
            let offset_addr = self.base_addr + ((bus_index as usize) << 20);
            let mmio_frames = unsafe {
                crate::memory::global_memory()
                    .acquire_frame(
                        offset_addr.as_usize() / 0x1000,
                        crate::memory::FrameState::MMIO,
                    )
                    .unwrap()
                    .into_iter()
            };

            let mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                .unwrap()
                .map();
            info!("{:?}", unsafe {
                &*mmio
                    .mapped_addr()
                    .as_ptr::<crate::io::pcie::PCIEDeviceHeader>()
            });
        }
    }
}
