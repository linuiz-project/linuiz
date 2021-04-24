use crate::{addr_ty::Physical, io::pci::PCIeBus, Address};
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

    unsafe fn configure_bus(
        &mut self,
        bus_index: u8,
        offset_addr: Address<Physical>,
    ) -> Result<(), PCIeHostBridgeError> {
        let bus = PCIeBus::new(offset_addr);

        if !bus.has_devices() {
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

    pub fn iter(&self) -> alloc::collections::btree_map::Iter<u8, PCIeBus> {
        self.busses.iter()
    }
}

static mut HOST_BRIDGES: BTreeMap<u16, PCIeHostBridge> = BTreeMap::new();

pub fn configure_host_bridge(
    entry: &crate::acpi::rdsp::xsdt::mcfg::MCFGEntry,
) -> Result<(), PCIeHostBridgeError> {
    if !entry.base_addr().is_canonical() {
        Err(PCIeHostBridgeError::InvalidBaseAddress(entry.base_addr()))
    } else if unsafe { HOST_BRIDGES.get(&entry.seg_group_num()).is_some() } {
        Err(PCIeHostBridgeError::BridgeConfigured(entry.seg_group_num()))
    } else {
        let mut bridge = PCIeHostBridge::empty();
        let bus_range = entry.start_pci_bus()..=entry.end_pci_bus();
        debug!(
            "Configuring express host bridge:\n Base Address: {:?}\n Segment Group: {}\n Bus Range: {:?}",
            entry.base_addr(),
            entry.seg_group_num(),
            bus_range
        );

        for bus_index in bus_range {
            unsafe {
                let offet_addr = entry.base_addr() + ((bus_index as usize) << 20);
                bridge.configure_bus(bus_index, offet_addr).ok();
            }
        }

        debug!(
            "Configured PCIe host bridge group {} with {} valid busses.",
            entry.seg_group_num(),
            bridge.busses.len()
        );

        unsafe {
            HOST_BRIDGES
                .insert(entry.seg_group_num(), bridge)
                .expect_none("attempted to insert PCI bridge for existent segment group")
        };

        Ok(())
    }
}

pub fn host_bridges() -> &'static BTreeMap<u16, PCIeHostBridge> {
    unsafe { &HOST_BRIDGES }
}
