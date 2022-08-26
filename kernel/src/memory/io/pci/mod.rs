mod device;

pub use device::*;

pub fn get_pcie_devices<'a>() -> impl Iterator<Item = DeviceVariant> + 'a {
    crate::acpi::xsdt::get_xsdt()
        .find_sub_table::<crate::acpi::xsdt::mcfg::MCFG>()
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
        .flat_map(|bus_base_addr| {
            (0..32).map(move |device_index| bus_base_addr + (device_index << 15))
        })
        .filter_map(move |device_base_addr| unsafe {
            let mmio = crate::memory::MMIO::new((device_base_addr).frame_index(), 1);

            let vendor_id = mmio.read::<u16>(0).assume_init();

            if vendor_id > u16::MIN && vendor_id < u16::MAX {
                debug!("Configuring PCIe bus: @{:?}", device_base_addr);

                let page_manager = crate::memory::global_pmgr();
                for page in mmio.pages() {
                    page_manager.set_page_attribs(
                        &page,
                        {
                            use crate::memory::PageAttributes;

                            PageAttributes::DATA
                                | PageAttributes::WRITE_THROUGH
                                | PageAttributes::UNCACHEABLE
                        },
                        crate::memory::AttributeModify::Set,
                    );
                }
            }

            Some(crate::io::pci::new_device(mmio))
        })
}
