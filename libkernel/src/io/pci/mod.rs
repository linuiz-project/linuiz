mod bus;
mod device;
mod host;

pub use bus::*;
pub use device::*;
pub use host::*;

pub fn get_pcie_devices(
    page_manager: Option<&crate::memory::PageManager>,
) -> impl Iterator<Item = DeviceVariant> + '_ {
    crate::acpi::rdsp::xsdt::XSDT
        .find_sub_table::<crate::acpi::rdsp::xsdt::mcfg::MCFG>()
        .expect("XSDT does not contain an MCFG table")
        .iter()
        .filter_map(move |entry| {
            if entry.base_addr().is_canonical() {
                Some(entry)
            } else {
                None
            }
        })
        .flat_map(|entry| {
            (entry.start_pci_bus()..=entry.end_pci_bus())
                .map(|bus_index| entry.base_addr() + ((bus_index as usize) << 20))
        })
        .flat_map(|bus_base_addr| (0..32).map(move |device_index| bus_base_addr + (device_index << 15)))
        .filter_map(move |device_base_addr| unsafe {
            let mmio = crate::memory::MMIO::new((device_base_addr).frame_index(), 1)
                .expect("Allocation error occurred attempting to create MMIO for PCIeBus");

            let vendor_id = mmio.read::<u16>(0).assume_init();

            if vendor_id > u16::MIN && vendor_id < u16::MAX {
                trace!("Configuring PCIe bus: @{:?}", device_base_addr);

                if let Some(page_manager) = page_manager {
                    for page in mmio.pages() {
                        page_manager.set_page_attribs(
                            &page,
                            crate::memory::PageAttributes::MMIO,
                            crate::memory::AttributeModify::Set,
                        );
                    }
                }

                Some(crate::io::pci::new_device(mmio, None))
            } else {
                None
            }
        })
}
