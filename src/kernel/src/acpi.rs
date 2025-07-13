use crate::mem::Hhdm;
use acpi::AcpiTables;
use core::ptr::NonNull;

#[derive(Clone, Copy)]
pub struct Handler;

// Safety: Type contains no values.
unsafe impl Send for Handler {}

impl acpi::AcpiHandler for Handler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        trace!("ACPI: Requested physical mapping at:{physical_address:#X} (size:{size})");

        let virtual_address = NonNull::with_exposed_provenance(Hhdm::offset(physical_address));

        // Safety:
        //  - `physical_address` is the physical address of the mapping.
        //  - `virtual_address` is the virtual address of the mapping.
        //  - `size` is both the requested and fulfilled size of the mapping.
        //  - Physical memory can always be mapped, as the higher-half direct map represents all physical memory.
        unsafe { acpi::PhysicalMapping::new(physical_address, virtual_address, size, size, Self) }
    }

    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {
        //  We don't actually need to unmap anything, since this utilizes the HHDM.
    }
}

pub fn get_tables(rsdp_request: &limine::request::RsdpRequest) -> AcpiTables<Handler> {
    let rsdp_response = rsdp_request
        .get_response()
        .expect("bootloader did not provide a response to RSDP address request");

    // Safety: Bootloader guarantees provided RSDP address to be valid.
    (unsafe { AcpiTables::from_rsdp(Handler, rsdp_response.address()) })
        .expect("ACPI table validation failed")
}
