use crate::mem::HigherHalfDirectMap;
use acpi::{AcpiError, AcpiTables};
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
        trace!("Physical mapping @ {physical_address:#X} (size:{size})");

        let virtual_address =
            NonNull::with_exposed_provenance(HigherHalfDirectMap::offset(physical_address));

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

#[derive(Debug, Error)]
pub enum Error {
    #[error("bootloader did not provide an RSDP address")]
    NoRsdpAddress,

    #[error("failed to validate ACPI root table")]
    ValidationFailed(AcpiError),
}

impl From<AcpiError> for Error {
    fn from(error: AcpiError) -> Self {
        Self::ValidationFailed(error)
    }
}

pub fn get_root_table(
    rsdp_request: &limine::request::RsdpRequest,
) -> Result<AcpiTables<Handler>, Error> {
    let rsdp_response = rsdp_request.get_response().ok_or(Error::NoRsdpAddress)?;

    let rsdp_address = rsdp_response.address();
    debug!("Found RSDP: {rsdp_address:#X?}");

    let physical_address = {
        // Limine protocol specification states that base revisions < 3 provides
        // the RSDP address as a virtual address rather than physical.
        if rsdp_response.revision() < 3 {
            HigherHalfDirectMap::negative_offset(rsdp_address).get()
        } else {
            rsdp_address
        }
    };

    // Safety: Bootloader guarantees provided RSDP address to be valid.
    let root_table = unsafe { AcpiTables::from_rsdp(Handler, physical_address) }?;

    Ok(root_table)
}
