use core::ops::RangeInclusive;

use crate::{io::pci::PCIeBus, Address, Physical};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum PCIeHostBridgeError {
    NonCanonicalBaseAddress(Address<Physical>),
    BusConfigured(u8),
    BusInvalid(u8),
}

#[derive(Debug)]
pub struct PCIeHostBridge {
    busses: Vec<PCIeBus>,
}

impl PCIeHostBridge {
    pub fn new(
        base_addr: Address<Physical>,
        bus_range: RangeInclusive<u8>,
    ) -> Self {
        Self {
            busses: bus_range
                .filter_map(|bus_index| {
                    let bus = unsafe {
                        PCIeBus::new(base_addr + ((bus_index as usize) << 20))
                    };

                    if bus.has_devices() {
                        Some(bus)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    pub fn iter(&self) -> core::slice::Iter<PCIeBus> {
        self.busses.iter()
    }
}

pub fn configure_host_bridge(
    entry: &crate::acpi::rdsp::xsdt::mcfg::Entry,
) -> Result<PCIeHostBridge, PCIeHostBridgeError> {
    debug!(
        "Configuring PCIe host bridge for bus range: {:?}",
        entry.start_pci_bus()..=entry.end_pci_bus()
    );

    if !entry.base_addr().is_canonical() {
        Err(PCIeHostBridgeError::NonCanonicalBaseAddress(
            entry.base_addr(),
        ))
    } else {
        let bridge = PCIeHostBridge::new(
            entry.base_addr(),
            entry.start_pci_bus()..=entry.end_pci_bus(),
        );

        trace!(
            "Successfully configured PCIe host bridge: {} live busses",
            bridge.busses.len()
        );

        Ok(bridge)
    }
}
