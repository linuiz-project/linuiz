use crate::Address;
use acpi::{fadt::Fadt, mcfg::Mcfg, sdt::Signature, AcpiTables, PhysicalMapping, PlatformInfo};
use port::{PortAddress, ReadOnlyPort, ReadWritePort, WriteOnlyPort};
use spin::{Mutex, MutexGuard, Once};

pub enum Register<'a, T: port::PortReadWrite> {
    IO(ReadWritePort<T>),
    MMIO(&'a crate::memory::VolatileCell<T, crate::ReadWrite>),
}

impl<T: port::PortReadWrite> Register<'_, T> {
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
                        ReadWritePort::<T>::new(generic_address.address as PortAddress)
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
    unsafe fn map_physical_region<T>(&self, address: usize, size: usize) -> acpi::PhysicalMapping<Self, T> {
        let kernel_allocator = crate::memory::get_global_allocator();
        if address > kernel_allocator.total_memory() {
            panic!("physical region address out of memory range")
        } else {
            let aligned_address = Address::<crate::Frame>::new_truncate(address as u64);
            let aligned_size = (address & !0xFFF).abs_diff(address + size);
            let frame_count = (aligned_size / 0x1000) + 1;
            let virtual_address = kernel_allocator.allocate_to(aligned_address, frame_count).unwrap();

            acpi::PhysicalMapping::new(
                aligned_address.as_usize(),
                core::ptr::NonNull::new(virtual_address.as_mut_ptr()).unwrap(),
                size,
                frame_count * 0x1000,
                Self,
            )
        }
    }

    fn unmap_physical_region<T>(_: &acpi::PhysicalMapping<Self, T>) {
        // ... We don't actually need to unmap anything, since this utilizes the HHDM
    }
}

#[allow(clippy::undocumented_unsafe_blocks)]
impl aml::Handler for AcpiHandler {
    fn read_u8(&self, address: usize) -> u8 {
        unsafe { (address as *const u8).read() }
    }

    fn read_u16(&self, address: usize) -> u16 {
        unsafe { (address as *const u16).read() }
    }

    fn read_u32(&self, address: usize) -> u32 {
        unsafe { (address as *const u32).read() }
    }

    fn read_u64(&self, address: usize) -> u64 {
        unsafe { (address as *const u64).read() }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unsafe { (address as *mut u8).write(value) };
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        unsafe { (address as *mut u16).write(value) };
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        unsafe { (address as *mut u32).write(value) };
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        unsafe { (address as *mut u64).write(value) };
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

    fn read_pci_u8(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16) -> u8 {
        todo!()
    }

    fn read_pci_u16(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16) -> u16 {
        todo!()
    }

    fn read_pci_u32(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16) -> u32 {
        todo!()
    }

    fn write_pci_u8(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u8) {
        todo!()
    }

    fn write_pci_u16(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u16) {
        todo!()
    }

    fn write_pci_u32(&self, _segment: u16, _bus: u8, _device: u8, _function: u8, _offset: u16, _value: u32) {
        todo!()
    }
}

struct AcpiTablesWrapper(AcpiTables<AcpiHandler>);
// SAFETY: Read-only type.
unsafe impl Sync for AcpiTablesWrapper {}

static RSDP: Once<Mutex<AcpiTables<AcpiHandler>>> = Once::new();
/// Initializes the ACPI interface.
///
/// SAFETY: This this method must be called before bootloader memory is reclaimed.
pub unsafe fn init_interface(rsdp_address: Address<crate::Physical>) {
    RSDP.call_once(move || {
        Mutex::new({
            let handler = AcpiHandler;

            // SAFETY:  We simply have no way to check if the bootloader provides an invalid RSDP address.
            //          Hopefully, the crate's safety checks catch it.
            unsafe {
                acpi::AcpiTables::from_rsdp(handler, rsdp_address.as_usize()).expect("failed to acquire RSDP table")
            }
        })
    });
}

pub fn get_rsdp() -> MutexGuard<'static, acpi::AcpiTables<AcpiHandler>> {
    RSDP.get().expect("RSDP has not been initialized").lock()
}

static PLATFORM_INFO: Once<Mutex<PlatformInfo>> = Once::new();
/// Returns an insatnce of the machine's MADT, or panics of it isn't present.
pub fn get_platform_info() -> MutexGuard<'static, PlatformInfo> {
    PLATFORM_INFO
        .call_once(|| Mutex::new(PlatformInfo::new(&*get_rsdp()).expect("error parsing machine platform info")))
        .lock()
}

static FADT: Once<Mutex<PhysicalMapping<AcpiHandler, Fadt>>> = Once::new();
/// Returns an instance of the machine's FADT, or panics if it isn't present.
pub fn get_fadt() -> MutexGuard<'static, PhysicalMapping<AcpiHandler, Fadt>> {
    FADT.call_once(|| {
        Mutex::new({
            let rsdp = get_rsdp();

            // SAFETY: Using the `Fadt` type from the `acpi` crate, we can be certain the SDT's structure will match the memory the crate wraps.
            unsafe {
                rsdp.get_sdt::<Fadt>(Signature::FADT)
                    .expect("FADT failed to validate its checksum")
                    .expect("no FADT found in RSDP table")
            }
        })
    })
    .lock()
}

static MCFG: Once<Mutex<PhysicalMapping<AcpiHandler, Mcfg>>> = Once::new();
/// Returns an instance of the machine's MCFG, or panics if it isn't present.
pub fn get_mcfg() -> MutexGuard<'static, PhysicalMapping<AcpiHandler, Mcfg>> {
    MCFG.call_once(|| {
        Mutex::new({
            let rsdp = get_rsdp();

            // SAFETY: Using the `Mcfg` type from the `acpi` crate, we can be certain the SDT's structure will match the memory it wraps.
            unsafe {
                rsdp.get_sdt::<Mcfg>(Signature::MCFG)
                    .expect("MCFG failed to validate its checksum")
                    .expect("no MCFG found in RSDP table")
            }
        })
    })
    .lock()
}

// struct AmlContextWrapper(aml::AmlContext);
// // SAFETY: TODO
// unsafe impl Sync for AmlContextWrapper {}

// static AML_CONTEXT: Once<AmlContextWrapper> = Once::new();

// pub fn init_aml_context() {
//     AML_CONTEXT.call_once(|| {
//         AmlContextWrapper({
//             let mut aml_context = aml::AmlContext::new(alloc::boxed::Box::new(AcpiHandler), aml::DebugVerbosity::All);
//             let kernel_hhdm_address = crate::memory::get_kernel_hhdm_address().as_usize();
//             let rsdp = get_rsdp();

//             {
//                 let dsdt_table = rsdp.dsdt.as_ref().expect("machine has no DSDT");

//                 // SAFETY: We can be reasonably certain the provided base address and length are valid.
//                 let dsdt_stream = unsafe {
//                     core::slice::from_raw_parts(
//                         (dsdt_table.address + kernel_hhdm_address) as *const u8,
//                         dsdt_table.length as usize,
//                     )
//                 };

//                 aml_context.parse_table(dsdt_stream).expect("failed to parse DSDT");
//             }

//             {
//                 for sdst_table in &get_rsdp().ssdts {
//                     // SAFETY: We can be reasonably certain the provided base address and length are valid.
//                     let sdst_stream = unsafe {
//                         core::slice::from_raw_parts(
//                             (sdst_table.address + kernel_hhdm_address) as *const u8,
//                             sdst_table.length as usize,
//                         )
//                     };

//                     aml_context.parse_table(sdst_stream).expect("failed to parse SDST");
//                 }
//             }

//             aml_context.initialize_objects().expect("failed to initialize AML objects");

//             aml_context
//         })
//     });
// }
