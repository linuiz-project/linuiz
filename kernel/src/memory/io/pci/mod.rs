mod device;

pub use device::*;

struct DeviceWrapper(DeviceVariant);
unsafe impl Send for DeviceWrapper {}
unsafe impl Sync for DeviceWrapper {}

static PCI_DEVICES: spin::Once<alloc::vec::Vec<DeviceWrapper>> = spin::Once::new();

pub fn init_pcie_devices() {
    let kernel_hhdm_address = crate::memory::get_kernel_hhdm_address();
    let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
    let kernel_page_manager = crate::memory::get_kernel_page_manager();

    PCI_DEVICES.call_once(|| {
        crate::tables::acpi::get_mcfg()
            .entries()
            .iter()
            .filter(|entry| libkernel::Address::<libkernel::Physical>::is_canonical(entry.base_address))
            .flat_map(|entry| {
                (entry.bus_number_start..=entry.bus_number_end)
                    .map(|bus_index| (entry.pci_segment_group, entry.base_address + ((bus_index as u64) << 20)))
            })
            .enumerate()
            .flat_map(|(bus_index, (segment_index, bus_base_addr))| {
                (0..32).map(move |device_index| {
                    (segment_index, bus_index as u16, device_index as u16, bus_base_addr + (device_index << 15))
                })
            })
            .filter_map(move |(segment_index, bus_index, device_index, device_base_addr)| unsafe {
                let device_frame_index = device_base_addr / 0x1000;
                let device_hhdm_page = libkernel::memory::Page::from_index(
                    (kernel_hhdm_address.as_usize() + (device_base_addr as usize)) / 0x1000,
                );

                kernel_page_manager
                    .map_mmio(device_hhdm_page, device_frame_index as usize, kernel_frame_manager)
                    .unwrap();

                let vendor_id = device_hhdm_page.as_ptr::<crate::num::LittleEndianU16>().read_volatile().get();
                if vendor_id > u16::MIN && vendor_id < u16::MAX {
                    debug!(
                        "Configuring PCIe device: [{:0>2}:{:0>2}:{:0>2}.00@{:#X}]",
                        segment_index, bus_index, device_index, device_base_addr
                    );

                    let pcie_device = new_device(device_hhdm_page.as_mut_ptr());
                    debug!("{:#?}", pcie_device);
                    Some(DeviceWrapper(pcie_device))
                } else {
                    kernel_page_manager.unmap(&device_hhdm_page, false, kernel_frame_manager).unwrap();

                    None
                }
            })
            .collect()
    });
}
