use acpi::{fadt::Fadt, sdt::Signature, AcpiTables, PhysicalMapping};
use spin::Once;

/// REMARK: Naming convention aligns with `acpi` crate convention.
#[derive(Clone, Copy)]
pub struct AcpiHandler;
impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> acpi::PhysicalMapping<Self, T> {
        // Ensure we modify memory manager state, to keep it consistent, and ACPI MMIO uncached.
        let page_manager = crate::memory::get_kernel_page_manager();
        for page_index in (libkernel::align_down(physical_address, 0x1000)
            ..libkernel::align_up(physical_address + size, 0x1000))
            .step_by(0x1000)
            .map(|addr| addr / 0x1000)
        {
            page_manager.set_page_attributes(
                &libkernel::memory::Page::from_index(page_index),
                libkernel::memory::PageAttributes::MMIO,
                libkernel::memory::AttributeModify::Set,
            );
        }

        acpi::PhysicalMapping::new(
            physical_address,
            core::ptr::NonNull::new_unchecked(physical_address as *mut _),
            size,
            size,
            Self,
        )
    }

    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {
        // ... We don't actually need to unmap anything, since this utilizes the HHDM
    }
}

static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(crate::LIMINE_REV);

struct AcpiTablesWrapper(AcpiTables<AcpiHandler>);
unsafe impl Send for AcpiTablesWrapper {}
unsafe impl Sync for AcpiTablesWrapper {}

static RSDP: Once<AcpiTablesWrapper> = Once::new();
fn get_rsdp() -> &'static acpi::AcpiTables<AcpiHandler> {
    &RSDP
        .call_once(|| unsafe {
            let rsdp_ptr = LIMINE_RSDP
                .get_response()
                .get()
                .expect("bootloader failed to provide an RSDP address")
                .address
                .as_ptr()
                .expect("bootloader RSDP address is not valid");
            debug!("RSDP pointer is: {:?}", rsdp_ptr);

            AcpiTablesWrapper(
                acpi::AcpiTables::from_rsdp(AcpiHandler, rsdp_ptr as usize).expect("failed to acquire RSDP table"),
            )
        })
        .0
}

struct FadtWrapper(PhysicalMapping<AcpiHandler, Fadt>);
unsafe impl Send for FadtWrapper {}
unsafe impl Sync for FadtWrapper {}

static FADT: Once<FadtWrapper> = Once::new();
/// Returns an instance of the machine's FADT, or panics if it isn't present.
pub fn get_fadt() -> &'static PhysicalMapping<AcpiHandler, Fadt> {
    &FADT
        .call_once(|| unsafe {
            FadtWrapper(
                get_rsdp()
                    .get_sdt::<acpi::fadt::Fadt>(Signature::FADT)
                    .expect("FADT failed to validate its checksum")
                    .expect("no FADT found in RSDP table"),
            )
        })
        .0
}
