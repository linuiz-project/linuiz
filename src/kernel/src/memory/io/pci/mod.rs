mod device;

pub use device::*;

use alloc::vec::Vec;
use libkernel::{sync::SingleOwner, Address, Physical};
use spin::RwLock;

static PCI_DEVICES: RwLock<Vec<SingleOwner<Device<Standard>>>> = RwLock::new(Vec::new());

pub const fn get_device_base_address(base: u64, bus_index: u8, device_index: u8) -> Address<Physical> {
    Address::<Physical>::new_truncate(base + (((bus_index as u64) << 20) | ((device_index as u64) << 15)))
}

pub fn init_devices() {
    let kernel_hhdm_address = crate::memory::get_kernel_hhdm_address();
    let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
    let kernel_page_manager = crate::memory::get_kernel_page_manager();
    let mut pci_devices = PCI_DEVICES.write();

    crate::tables::acpi::get_mcfg()
        .entries()
        .iter()
        .filter(|entry| libkernel::Address::<libkernel::Physical>::is_canonical(entry.base_address))
        .flat_map(|entry| {
            // Enumerate buses
            (entry.bus_number_start..=entry.bus_number_end)
                .map(|bus_index| (entry.base_address, entry.pci_segment_group, bus_index))
        })
        .flat_map(|(base_address, segment_index, bus_index)| {
            // Enumerate devices
            (0..32).map(move |device_index| (base_address, segment_index, bus_index, device_index as u8))
        })
        .for_each(move |(base_address, segment_index, bus_index, device_index)| unsafe {
            // Allocate devices
            let device_base_address = get_device_base_address(base_address, bus_index, device_index);
            let device_hhdm_page = libkernel::memory::Page::from_index(
                kernel_hhdm_address.page_index() + device_base_address.frame_index(),
            );

            kernel_page_manager
                .map_mmio(device_hhdm_page, device_base_address.frame_index(), kernel_frame_manager)
                .unwrap();

            let vendor_id = device_hhdm_page.address().as_ptr::<crate::num::LittleEndianU16>().read_volatile().get();
            if vendor_id > u16::MIN && vendor_id < u16::MAX {
                debug!(
                    "Configuring PCIe device: [{:0>2}:{:0>2}:{:0>2}.00@{:?}]",
                    segment_index, bus_index, device_index, device_base_address
                );

                if let DeviceVariant::Standard(pci_device) = new_device(device_hhdm_page.address().as_mut_ptr()) {
                    trace!("{:#?}", pci_device);
                    pci_devices.push(SingleOwner::new(pci_device));
                }
                // TODO handle PCI-to-PCI busses
            } else {
                // Unmap the unused device MMIO
                kernel_page_manager.unmap(&device_hhdm_page, false, kernel_frame_manager).unwrap();
            }
        })
}
