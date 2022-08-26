use crate::memory::io::{PortAddress, ReadOnlyPort, WriteOnlyPort};
use crate::memory::{ensure_hhdm_frame_is_mapped, get_kernel_hhdm_address, PageAttributes};
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
        // The RSDP address provided by Limine resides within the HHDM, but the other pointers do not. This logic
        // accounts for that quirk.
        let hhdm_physical_address =
            if physical_address > hhdm_base_address { physical_address } else { hhdm_base_address + physical_address };

        let kernel_frame_manager = crate::memory::get_kernel_frame_manager();
        let kernel_page_manager = crate::memory::get_kernel_page_manager();
        for page_base in (hhdm_physical_address..(hhdm_physical_address + size)).step_by(0x1000) {
            let page = libkernel::memory::Page::from_index(page_base / 0x1000);

            if !kernel_page_manager.is_mapped(page) {
                kernel_page_manager
                    .map(
                        &page,
                        physical_address / 0x1000,
                        false,
                        crate::memory::PageAttributes::RW,
                        kernel_frame_manager,
                    )
                    .unwrap();
            }
        }

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
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *const u8).read() }
    }

    fn read_u16(&self, address: usize) -> u16 {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *const u16).read() }
    }

    fn read_u32(&self, address: usize) -> u32 {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *const u32).read() }
    }

    fn read_u64(&self, address: usize) -> u64 {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *const u64).read() }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *mut u8).write(value) };
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *mut u16).write(value) };
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *mut u32).write(value) };
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        ensure_hhdm_frame_is_mapped(address / 0x1000, PageAttributes::MMIO);
        unsafe { ((address + get_kernel_hhdm_address().as_usize()) as *mut u64).write(value) };
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

pub fn init_aml_context() {
    AML_CONTEXT.call_once(|| {
        AmlContextWrapper({
            let mut aml_context = aml::AmlContext::new(alloc::boxed::Box::new(AcpiHandler), aml::DebugVerbosity::All);
            let kernel_hhdm_address = crate::memory::get_kernel_hhdm_address().as_usize();

            {
                let dsdt_table = get_rsdp().dsdt.as_ref().expect("machine has no DSDT");

                // SAFETY: We can be reasonably certain the provided base address and length are valid.
                let dsdt_stream = unsafe {
                    core::slice::from_raw_parts(
                        (dsdt_table.address + kernel_hhdm_address) as *const u8,
                        dsdt_table.length as usize,
                    )
                };

                debug!("Parsing DSDT @{:?}", dsdt_stream.as_ptr());
                aml_context.parse_table(dsdt_stream).expect("failed to parse DSDT");
            }

            {
                for sdst_table in &get_rsdp().ssdts {
                    // SAFETY: We can be reasonably certain the provided base address and length are valid.
                    let sdst_stream = unsafe {
                        core::slice::from_raw_parts(
                            (sdst_table.address + kernel_hhdm_address) as *const u8,
                            sdst_table.length as usize,
                        )
                    };

                    debug!("Parsing SDST @{:?}", sdst_stream.as_ptr());
                    aml_context.parse_table(sdst_stream).expect("failed to parse SDST");
                }
            }

            aml_context.initialize_objects().expect("failed to initialize AML objects");

            aml_context
        })
    });
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PM1a_EVT_BLK : u32 {
        /* statuses */
        const STATUS_TIMER = 1 << 0;
        const STATUS_BUS_MASTER = 1 << 4;
        const STATUS_GLOBAL =  1 << 5;
        const STATUS_POWER_BUTTON =  1 << 8;
        const STATUS_SLEEP_BUTTON =  1 << 9;
        const STATUS_RTC =  1 << 10;
        const STATUS_PCIe_WAKE =  1 << 14;
        const STATUS_WAKE =  1 << 15;

        /* enables */
        const ENABLE_TIMER = 1 << 16;
        const ENABLE_GLOBAL = 1 << 21;
        const ENABLE_POWER_BUTTON = 1 << 24;
        const ENABLE_SLEEP_BUTTON = 1 << 25;
        const ENABLE_RTC = 1 << 26;
        const ENABLE_PCIe_WAKE =  1 << 30;
    }
}

use crate::memory::io;

impl io::PortRead for PM1a_EVT_BLK {
    unsafe fn read(port: PortAddress) -> Self {
        Self::from_bits_truncate(io::_read32(port))
    }
}

impl io::PortWrite for PM1a_EVT_BLK {
    unsafe fn write(port: PortAddress, value: Self) {
        io::_write32(port, value.bits());
    }
}

impl io::PortReadWrite for PM1a_EVT_BLK {}
