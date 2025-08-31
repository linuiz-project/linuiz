use crate::{acpi::rsdt::SdtVariant, util::AsciiStr};
use core::ptr::NonNull;
use limine::request::RsdpRequest;

pub mod fadt;
pub mod rsdp;
pub mod rsdt;
pub mod waet;

mod address;
pub use address::GenericAddress;

#[repr(C, packed)]
struct SystemDescriptorTableHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

unsafe trait SystemDescriptorTable {
    const SIGNATURE: AsciiStr<4>;

    fn base_ptr(&self) -> NonNull<u8>;

    unsafe fn read_offset_as<T>(&self, offset: usize) -> T {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            self.base_ptr()
                .byte_add(offset)
                .cast::<T>()
                .read_unaligned()
        }
    }

    fn signature(&self) -> AsciiStr<4> {
        let bytes = unsafe { self.read_offset_as::<[u8; 4]>(0) };
        AsciiStr::new_lossy(bytes)
    }

    fn length(&self) -> usize {
        let length = unsafe { self.read_offset_as::<u32>(4) };
        usize::try_from(length).unwrap()
    }

    fn oem_id(&self) -> AsciiStr<6> {
        let bytes = unsafe { self.read_offset_as::<[u8; 6]>(10) };
        AsciiStr::new_lossy(bytes)
    }

    fn oem_table_id(&self) -> AsciiStr<8> {
        let bytes = unsafe { self.read_offset_as::<[u8; 8]>(16) };
        AsciiStr::new_lossy(bytes)
    }

    fn oem_revision(&self) -> u32 {
        unsafe { self.read_offset_as::<u32>(24) }
    }

    fn creator_id(&self) -> AsciiStr<4> {
        let bytes = unsafe { self.read_offset_as::<[u8; 4]>(28) };
        AsciiStr::new_lossy(bytes)
    }

    fn creator_revision(&self) -> u32 {
        unsafe { self.read_offset_as::<u32>(32) }
    }

    fn validate_checksum(&self) -> bool {
        let bytes = unsafe { core::slice::from_raw_parts(self.base_ptr().as_ptr(), self.length()) };
        let checksum = bytes.iter().copied().fold(0u8, u8::wrapping_add);

        checksum == 0
    }

    fn write_header_debug_fields(&self, d: &mut core::fmt::DebugStruct) {
        d.field("Signature", &self.signature().as_str())
            .field("OEM ID", &self.oem_id())
            .field("OEM Table ID", &self.oem_table_id().as_str())
            .field("OEM Revision", &self.oem_revision())
            .field("Creator ID", &self.creator_id());
    }
}

pub fn init_tables(rsdp_request: &RsdpRequest) {
    let rsdp_response = rsdp_request
        .get_response()
        .expect("bootloader did not respond to RSDP request");
    debug!("ACPI RSDP address: {:#X}", rsdp_response.address());

    // Safety: Bootloader guarantees root system descriptor pointer is valid.
    let rsdp = unsafe { rsdp::Rsdp::from_address(rsdp_response.address()) };

    if !rsdp.is_checksum_valid() {
        error!("ACPI RSDP checksum failed validation.");
        return;
    }

    debug!("{rsdp:#?}");

    match rsdp.get_rsdt() {
        rsdp::RsdtVariant::Rsdt(rsdt) => {
            debug!("{rsdt:#?}");

            for_each_sdt(rsdt.entries());
        }
        rsdp::RsdtVariant::Xsdt(xsdt) => {
            debug!("{xsdt:#?}");

            for_each_sdt(xsdt.entries());
        }
    }
}

fn for_each_sdt(entries: impl Iterator<Item = SdtVariant>) {
    entries.for_each(|sdt| {
        debug!("{sdt:#?}");

        match sdt {
            SdtVariant::Fadt(fadt) => {
                crate::time::KernelStopwatch::init(fadt.pm_timer());
            }

            _ => {}
        }
    });
}
