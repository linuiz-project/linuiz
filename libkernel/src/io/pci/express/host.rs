use crate::{addr_ty::Physical, io::pci::express::PCIeBus, Address};
use alloc::collections::BTreeMap;

#[derive(Debug)]
pub enum PCIeHostBridgeError {
    InvalidBaseAddress(Address<Physical>),
    BridgeConfigured(u16),
    BusConfigured(u8),
    BusInvalid(u8),
}

#[derive(Debug)]
pub struct PCIeHostBridge {
    busses: BTreeMap<u8, PCIeBus>,
}

impl PCIeHostBridge {
    fn empty() -> Self {
        Self {
            busses: BTreeMap::new(),
        }
    }

    fn configure_bus(
        &mut self,
        bus_index: u8,
        offset_addr: Address<Physical>,
    ) -> Result<(), PCIeHostBridgeError> {
        let bus = unsafe { PCIeBus::new(offset_addr) };

        if !bus.is_valid() {
            Err(PCIeHostBridgeError::BusInvalid(bus_index))
        } else {
            self.busses.insert(bus_index, bus).map_or(Ok(()), |_| {
                Err(PCIeHostBridgeError::BusConfigured(bus_index))
            })
        }
    }

    pub fn get_bus(&self, bus_index: u8) -> Option<&PCIeBus> {
        self.busses.get(&bus_index)
    }
}

static mut PCIE_HOST_BRIDGES: BTreeMap<u16, PCIeHostBridge> = BTreeMap::new();

pub fn configure_host_bridge(
    entry: &crate::acpi::rdsp::xsdt::mcfg::MCFGEntry,
) -> Result<(), PCIeHostBridgeError> {
    if !entry.base_addr().is_canonical() {
        Err(PCIeHostBridgeError::InvalidBaseAddress(entry.base_addr()))
    } else if unsafe { PCIE_HOST_BRIDGES.get(&entry.seg_group_num()).is_some() } {
        Err(PCIeHostBridgeError::BridgeConfigured(entry.seg_group_num()))
    } else {
        let mut bridge = PCIeHostBridge::empty();
        let bus_range = entry.start_pci_bus()..=entry.end_pci_bus();
        debug!("Configuring express host bridge:\n Base Address: {:?}\n Segment Group: {}\n Bus Range: {:?}",entry.base_addr(),entry.seg_group_num(),bus_range);

        bus_range.for_each(|bus_index| {
            bridge
                .configure_bus(bus_index, entry.base_addr() + ((bus_index as usize) << 20))
                .ok();
        });

        debug!("PCIe Host Bridge Busses:\n {:#?}", bridge.busses);

        unsafe {
            PCIE_HOST_BRIDGES
                .insert(entry.seg_group_num(), bridge)
                .unwrap_none()
        };

        Ok(())
    }
}
