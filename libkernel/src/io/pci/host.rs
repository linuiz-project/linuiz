use core::ops::RangeInclusive;

use crate::{addr_ty::Physical, io::pci::PCIeBus, Address};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum PCIeHostBridgeError {
    InvalidBaseAddress(Address<Physical>),
    BusConfigured(u8),
    BusInvalid(u8),
}

#[derive(Debug)]
pub struct PCIeHostBridge {
    seg_group: u16,
    min_bus: u8,
    busses: Vec<PCIeBus>,
}

impl PCIeHostBridge {
    pub fn new(
        seg_group: u16,
        base_addr: Address<Physical>,
        bus_range: RangeInclusive<u8>,
    ) -> Self {
        Self {
            seg_group,
            min_bus: *bus_range.start(),
            busses: bus_range
                .filter_map(|bus_index| {
                    let bus = unsafe { PCIeBus::new(base_addr + ((bus_index as usize) << 20)) };

                    if !bus.has_devices() {
                        warn!(
                            "PCIe segment group {}, bus {}, is invalid (has no devices). Skipping.",
                            seg_group, bus_index
                        );

                        None
                    } else {
                        Some(bus)
                    }
                })
                .collect(),
        }
    }

    pub const fn segment_group(&self) -> u16 {
        self.seg_group
    }

    pub fn iter(&self) -> core::slice::Iter<PCIeBus> {
        self.busses.iter()
    }
}

pub fn configure_host_bridge(
    entry: &crate::acpi::rdsp::xsdt::mcfg::MCFGEntry,
) -> Result<PCIeHostBridge, PCIeHostBridgeError> {
    if !entry.base_addr().is_canonical() {
        Err(PCIeHostBridgeError::InvalidBaseAddress(entry.base_addr()))
    } else {
        let bus_range = entry.start_pci_bus()..=entry.end_pci_bus();
        debug!(
            "Configuring express host bridge:\n Base Address: {:?}\n Segment Group: {}\n Bus Range: {:?}",
            entry.base_addr(),
            entry.seg_group_num(),
            bus_range
        );

        let bridge = PCIeHostBridge::new(entry.seg_group_num(), entry.base_addr(), bus_range);

        debug!(
            "Configured PCIe host bridge group {} with {} valid busses.",
            entry.seg_group_num(),
            bridge.busses.len()
        );

        Ok(bridge)
    }
}
