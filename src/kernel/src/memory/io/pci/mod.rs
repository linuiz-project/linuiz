mod device;

pub use device::*;

use alloc::vec::Vec;
use libcommon::{sync::SingleOwner, Address, Physical};
use spin::RwLock;

static PCI_DEVICES: RwLock<Vec<SingleOwner<Device<Standard>>>> = RwLock::new(Vec::new());

pub const fn get_device_base_address(base: u64, bus_index: u8, device_index: u8) -> Address<Physical> {
    Address::<Physical>::new_truncate(base + (((bus_index as u64) << 20) | ((device_index as u64) << 15)))
}

pub fn init_devices() {
    let kernel_hhdm_address = crate::memory::get_hhdm_address();
    let kernel_page_manager = crate::memory::get_kernel_virtual_mapper();
    let mut pci_devices = PCI_DEVICES.write();

    crate::tables::acpi::get_mcfg()
        .entries()
        .iter()
        .filter(|entry| libcommon::Address::<libcommon::Physical>::is_canonical(entry.base_address))
        .flat_map(|entry| {
            // Enumerate buses
            (entry.bus_number_start..=entry.bus_number_end)
                .map(|bus_index| (entry.base_address, entry.pci_segment_group, bus_index))
        })
        .flat_map(|(base_address, segment_index, bus_index)| {
            // Enumerate devices
            (0..32).map(move |device_index| (base_address, segment_index, bus_index, device_index as u8))
        })
        .for_each(|(base_address, segment_index, bus_index, device_index)| {
            // Allocate devices
            let device_base_address = get_device_base_address(base_address, bus_index, device_index);
            // TODO somehow test if the computed base address makes any sense.
            let Some(device_hhdm_page) = Address::<libcommon::Page>::new(kernel_hhdm_address.as_u64(), libcommon::PageAlign::Align4KiB)
                .and_then(|page| page.forward_checked(device_base_address.frame_containing().index()))
                else {
                    warn!("Failed to map device HHDM page due to overflow: {:0>2}:{:0>2}:{:0>2}.00@{:?}",
                    segment_index, bus_index, device_index, device_base_address);

                    return;
                };

            // ### Safety: HHDM is guaranteed to be valid by the kernel.
            //         Additionally, if this mapping is invalid, it will be unmapped later on in this context.
            unsafe {
                kernel_page_manager
                    .map_mmio(device_hhdm_page, device_base_address.frame_containing(), kernel_frame_manager)
                    .unwrap()
            };

            // ### Safety: We should be reading known-good memory here, according to the PCI spec. The following `if` test will verify that.
            let vendor_id =
                unsafe { device_hhdm_page.address().as_ptr::<crate::num::LittleEndianU16>().read_volatile() }.get();
            if vendor_id > u16::MIN && vendor_id < u16::MAX {
                debug!(
                    "Configuring PCIe device: [{:0>2}:{:0>2}:{:0>2}.00@{:?}]",
                    segment_index, bus_index, device_index, device_base_address
                );

                if let DeviceVariant::Standard(pci_device) =
                    // ### Safety: Base pointer, at this point, has been verified as known-good.
                    unsafe { new_device(device_hhdm_page.address().as_mut_ptr()) }
                {
                    trace!("{:#?}", pci_device);
                    pci_devices.push(SingleOwner::new(pci_device));
                }
                // TODO handle PCI-to-PCI busses
            } else {
                // Unmap the unused device MMIO
                // ### Safety: HHDM is guaranteed to be valid by the kernel.
                //         Additionally, this page was just previously mapped, and so is a known-valid mapping (hence the `.unwrap()`).
                unsafe { kernel_page_manager.unmap(device_hhdm_page, false, kernel_frame_manager).unwrap() };
            }
        });
}
