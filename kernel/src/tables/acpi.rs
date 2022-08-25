use crate::memory::get_kernel_hhdm_address;
use crate::memory::io::{PortAddress, ReadOnlyPort, WriteOnlyPort};
use acpi::{fadt::Fadt, sdt::Signature, AcpiTables, PhysicalMapping, PlatformInfo};
use spin::Once;

pub enum Register<'a, T: crate::memory::io::PortReadWrite> {
    IO(crate::memory::io::ReadWritePort<T>),
    MMIO(&'a libkernel::memory::volatile::VolatileCell<T, libkernel::ReadWrite>),
}

impl<T: crate::memory::io::PortReadWrite> Register<'_, T> {
    pub const fn new(generic_address: &acpi::platform::address::GenericAddress) -> Option<Self> {
        match generic_address.address_space {
            acpi::platform::address::AddressSpace::SystemMemory => {
                Some(Self::MMIO(
                    // SAFETY: There's no meaningful way to validate the address provided by the `GenericAddress` structure.
                    unsafe { &*(generic_address.address as *const _) },
                ))
            }

            acpi::platform::address::AddressSpace::SystemIo => {
                Some(Self::IO(
                    // SAFETY: There's no meaningful way to validate the port provided by the `GenericAddress` structure.
                    unsafe {
                        #[allow(clippy::cast_possible_truncation)]
                        crate::memory::io::ReadWritePort::<T>::new(generic_address.address as PortAddress)
                    },
                ))
            }

            _ => None,
        }
    }

    #[inline]
    pub fn read(&self) -> T {
        match self {
            Register::IO(port) => port.read(),
            Register::MMIO(addr) => addr.read(),
        }
    }

    #[inline]
    pub fn write(&mut self, value: T) {
        match self {
            Register::IO(port) => port.write(value),
            Register::MMIO(addr) => addr.write(value),
        }
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::module_name_repetitions)]
pub struct AcpiHandler;

impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> acpi::PhysicalMapping<Self, T> {
        let hhdm_base_address = get_kernel_hhdm_address().as_usize();
        let hhdm_physical_address =
        // The RSDP address provided by Limine resides within the HHDM, but the other pointers do not. This logic
        // accounts for that quirk.
            if physical_address > hhdm_base_address { physical_address } else { hhdm_base_address + physical_address };

        acpi::PhysicalMapping::new(
            physical_address,
            core::ptr::NonNull::new_unchecked(hhdm_physical_address as *mut _),
            size,
            size,
            Self,
        )
    }

    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {
        // ... We don't actually need to unmap anything, since this utilizes the HHDM
    }
}

#[allow(clippy::undocumented_unsafe_blocks)]
impl aml::Handler for AcpiHandler {
    fn read_u8(&self, address: usize) -> u8 {
        unsafe { (address as *const u8).add(get_kernel_hhdm_address().as_usize()).read() }
    }

    fn read_u16(&self, address: usize) -> u16 {
        unsafe { (address as *const u16).add(get_kernel_hhdm_address().as_usize()).read() }
    }

    fn read_u32(&self, address: usize) -> u32 {
        unsafe { (address as *const u32).add(get_kernel_hhdm_address().as_usize()).read() }
    }

    fn read_u64(&self, address: usize) -> u64 {
        unsafe { (address as *const u64).add(get_kernel_hhdm_address().as_usize()).read() }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unsafe { (address as *mut u8).add(get_kernel_hhdm_address().as_usize()).write(value) };
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        unsafe { (address as *mut u16).add(get_kernel_hhdm_address().as_usize()).write(value) };
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        unsafe { (address as *mut u32).add(get_kernel_hhdm_address().as_usize()).write(value) };
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        unsafe { (address as *mut u64).add(get_kernel_hhdm_address().as_usize()).write(value) };
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { ReadOnlyPort::<u8>::new(port as PortAddress) }.read()
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { ReadOnlyPort::<u16>::new(port as PortAddress) }.read()
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { ReadOnlyPort::<u32>::new(port as PortAddress) }.read()
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { WriteOnlyPort::<u8>::new(port as PortAddress) }.write(value);
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { WriteOnlyPort::<u16>::new(port as PortAddress) }.write(value);
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { WriteOnlyPort::<u32>::new(port as PortAddress) }.write(value);
    }

    fn read_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        todo!()
    }

    fn read_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        todo!()
    }

    fn read_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        todo!()
    }

    fn write_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u8) {
        todo!()
    }

    fn write_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u16) {
        todo!()
    }

    fn write_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u32) {
        todo!()
    }
}

static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(crate::LIMINE_REV);

struct AcpiTablesWrapper(AcpiTables<AcpiHandler>);
// SAFETY: Read-only type.
unsafe impl Send for AcpiTablesWrapper {}
// SAFETY: Read-only type.
unsafe impl Sync for AcpiTablesWrapper {}

static RSDP: Once<AcpiTablesWrapper> = Once::new();

/// Initializes the ACPI interface.
///
/// REMARK: If this method is called after bootloader memory has been reclaimed, it will panic.
pub fn init_interface() {
    RSDP.call_once(|| {
        AcpiTablesWrapper({
            let handler = AcpiHandler;
            let address = LIMINE_RSDP
                .get_response()
                .get()
                .expect("bootloader failed to provide an RSDP address")
                .address
                .as_ptr()
                .expect("bootloader RSDP address is not valid") as usize;

            // SAFETY:  We simply have no way to check if the bootloader provides an invalid RSDP address.
            //          Hopefully, the crate's safety checks catch it.
            unsafe { acpi::AcpiTables::from_rsdp(handler, address).expect("failed to acquire RSDP table") }
        })
    });
}

pub fn get_rsdp() -> &'static acpi::AcpiTables<AcpiHandler> {
    &RSDP.get().as_ref().unwrap().0
}

pub struct PlatformInfoWrapper(PlatformInfo);
// SAFETY: Read-only type.
unsafe impl Send for PlatformInfoWrapper {}
// SAFETY: Read-only type.
unsafe impl Sync for PlatformInfoWrapper {}

static PLATFORM_INFO: Once<PlatformInfoWrapper> = Once::new();
/// Returns an insatnce of the machine's MADT, or panics of it isn't present.
fn get_platform_info() -> &'static PlatformInfo {
    &PLATFORM_INFO
        .call_once(|| PlatformInfoWrapper(PlatformInfo::new(get_rsdp()).expect("error parsing machine platform info")))
        .0
}

static APIC_MODEL: Once<&'static acpi::platform::interrupt::Apic> = Once::new();
/// Returns the interrupt model of this machine.
pub fn get_apic_model() -> &'static acpi::platform::interrupt::Apic {
    APIC_MODEL.call_once(|| match &get_platform_info().interrupt_model {
        acpi::InterruptModel::Apic(apic) => apic,
        _ => panic!("unknown interrupt models not supported"),
    })
}

struct FadtWrapper(PhysicalMapping<AcpiHandler, Fadt>);
// SAFETY: Read-only type.
unsafe impl Send for FadtWrapper {}
// SAFETY: Read-only type.
unsafe impl Sync for FadtWrapper {}

static FADT: Once<FadtWrapper> = Once::new();
/// Returns an instance of the machine's FADT, or panics if it isn't present.
pub fn get_fadt() -> &'static PhysicalMapping<AcpiHandler, Fadt> {
    &FADT
        .call_once(|| {
            FadtWrapper({
                let rsdp = get_rsdp();

                // SAFETY: Using the `Fadt` type from the crate, we can be certain the SDT's structure will match the memory the crate wraps.
                unsafe {
                    rsdp.get_sdt::<Fadt>(Signature::FADT)
                        .expect("FADT failed to validate its checksum")
                        .expect("no FADT found in RSDP table")
                }
            })
        })
        .0
}

struct AmlContextWrapper(aml::AmlContext);
// SAFETY: TODO
unsafe impl Send for AmlContextWrapper {}
// SAFETY: TODO
unsafe impl Sync for AmlContextWrapper {}

static AML_CONTEXT: Once<AmlContextWrapper> = Once::new();

pub fn get_aml_context() -> &'static aml::AmlContext {
    &AML_CONTEXT
        .call_once(|| {
            AmlContextWrapper({
                let mut aml_context =
                    aml::AmlContext::new(alloc::boxed::Box::new(AcpiHandler), aml::DebugVerbosity::All);

                {
                    let dsdt_table = get_rsdp().dsdt.as_ref().expect("machine has no DSDT");

                    // SAFETY: We can be reasonably certain the provided base address and length are valid.
                    let dsdt_stream = unsafe {
                        core::slice::from_raw_parts(dsdt_table.address as *const u8, dsdt_table.length as usize)
                    };

                    aml_context.parse_table(dsdt_stream).expect("failed to parse DSDT");
                }

                {
                    for sdst_table in &get_rsdp().ssdts {
                        // SAFETY: We can be reasonably certain the provided base address and length are valid.
                        let sdst_stream = unsafe {
                            core::slice::from_raw_parts(sdst_table.address as *const u8, sdst_table.length as usize)
                        };
                        aml_context.parse_table(sdst_stream).expect("failed to parse SDST");
                    }
                }

                aml_context.initialize_objects().expect("failed to initialize AML objects");

                aml_context
            })
        })
        .0
}
