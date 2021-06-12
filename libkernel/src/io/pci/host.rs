use core::ops::RangeInclusive;

use crate::{addr_ty::Physical, io::pci::PCIeBus, Address};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum PCIeHostBridgeError {
    NonCanonicalBaseAddress(Address<Physical>),
    BusConfigured(u8),
    BusInvalid(u8),
}

#[derive(Debug)]
pub struct PCIeHostBridge {
    min_bus: u8,
    busses: Vec<PCIeBus>,
}

impl PCIeHostBridge {
    pub fn new(base_addr: Address<Physical>, bus_range: RangeInclusive<u8>) -> Self {
        Self {
            min_bus: *bus_range.start(),
            busses: bus_range
                .filter_map(|bus_index| {
                    let bus = unsafe { PCIeBus::new(base_addr + ((bus_index as usize) << 20)) };

                    if !bus.has_devices() {
                        None
                    } else {
                        Some(bus)
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
    entry: &crate::acpi::rdsp::xsdt::mcfg::MCFGEntry,
) -> Result<PCIeHostBridge, PCIeHostBridgeError> {
    debug!(
        "Attempting to configure a PCIe host bridge with MCFG entry:\n{:#?}",
        entry
    );

    if !entry.base_addr().is_canonical() {
        debug!("Failed to configure PCIe host bridge: non-canonical base address");

        Err(PCIeHostBridgeError::NonCanonicalBaseAddress(
            entry.base_addr(),
        ))
    } else {
        let bridge = PCIeHostBridge::new(
            entry.base_addr(),
            entry.start_pci_bus()..=entry.end_pci_bus(),
        );

        debug!(
            "Successfully configured PCIe host bridge ({} live busses).",
            bridge.busses.len()
        );

        Ok(bridge)
    }
}
