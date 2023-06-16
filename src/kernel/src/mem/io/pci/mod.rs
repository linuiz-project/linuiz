mod device;
pub use device::*;
use libkernel::LittleEndianU16;

use crate::mem::{
    alloc::pmm::PMM,
    paging::{self, TableDepth},
    with_kmapper, HHDM,
};
use alloc::{collections::BTreeMap, vec::Vec};
use libsys::{Address, Frame, Page, Physical};
use spin::{Mutex, RwLock};
use uuid::Uuid;

crate::error_impl! {
    #[derive(Debug)]
    pub enum Error {
        NoninitTables => None,
        AcpiError { err: acpi::AcpiError } => None,
        Paging { err: paging::Error } => Some(err)
    }
}

static PCI_DEVICES: Mutex<Vec<Device<Standard>>> = Mutex::new(Vec::new());
static OWNED_DEVICES: Mutex<BTreeMap<Uuid, Device<Standard>>> = Mutex::new(BTreeMap::new());

pub fn get_device_base_address(base: u64, bus_index: u8, device_index: u8) -> Address<Frame> {
    let bus_index = u64::from(bus_index);
    let device_index = u64::From(device_index);

    Address::new(base + (bus_index << 20) | (device_index << 15)).unwrap()
}

pub fn init_devices() -> Result<()> {
    with_kmapper(|kmapper| {
        let pci_devices = PCI_DEVICES.lock();

        let acpi_tables = crate::acpi::TABLES.get().ok_or(Error::NoninitTables)?.lock();
        let pci_regions = acpi::PciConfigRegions::new(&acpi_tables, &*PMM).map_err(|err| Error::AcpiError { err })?;

        pci_regions
            .iter()
            .filter_map(|entry| {
                Address::<Physical>::new(entry.physical_address)
                    .map(move |address| (address, entry.segment_group, entry.bus_range))
            })
            .flat_map(|(base_address, segment_index, bus_range)| {
                bus_range.map(move |bus_index| (base_address, segment_index, bus_index))
            })
            .flat_map(|(base_address, segment_index, bus_index)| {
                (0u8..32u8).map(move |device_index| (base_address, segment_index, bus_index, device_index))
            })
            .try_for_each(|(base_address, segment_index, bus_index, device_index)| {
                let device_base_address = get_device_base_address(base_address, bus_index, device_index);
                let device_page = HHDM.offset(device_base_address).unwrap();

                // Safety: HHDM is guaranteed to be valid by the kernel.
                //         Additionally, if this mapping is invalid, it will be unmapped later on in this context.
                unsafe {
                    kmapper.auto_map(device_page, paging::TableEntryFlags::RW)?;
                }

                // Safety: We should be reading known-good memory here, according to the PCI spec. The following `if` test will verify that.
                let vendor_id = unsafe { device_page.as_ptr().cast::<LittleEndianU16>().read_volatile() };
                if vendor_id > u16::MIN && vendor_id < u16::MAX {
                    debug!(
                        "Configuring PCIe device: [{:0>2}:{:0>2}:{:0>2}.00@{:?}]",
                        segment_index, bus_index, device_index, device_base_address
                    );

                    if let DeviceVariant::Standard(pci_device) =
                        // Safety: Base pointer, at this point, has been verified as known-good.
                        unsafe { new_device(device_page.as_ptr()) }
                    {
                        trace!("{:#?}", pci_device);
                        // pci_devices.push(SingleOwner::new(pci_device));
                    }
                    // TODO handle PCI-to-PCI busses
                } else {
                    // Unmap the unused device MMIO
                    // Safety: HHDM is guaranteed to be valid by the kernel.
                    //         Additionally, this page was just previously mapped, and so is a known-valid mapping (hence the `.unwrap()`).
                    unsafe {
                        kmapper.unmap(device_page, None, true);
                    }
                }
            })
    })?;

    Ok(())
}
