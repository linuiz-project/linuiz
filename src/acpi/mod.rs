use core::{fmt::Debug, str::Utf8Error};
use limine::request::RsdpRequest;

mod rsdp;

#[repr(transparent)]
struct Signature<const N: usize>([u8; N]);

impl Signature<4> {
    const RSDT: Self = Self(*b"RSDT");
    const XSDT: Self = Self(*b"XSDT");
}

impl<const N: usize> Signature<N> {
    pub fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub fn as_str(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(&self.0)
    }
}

impl<const N: usize> core::fmt::Debug for Signature<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<const N: usize> core::fmt::Display for Signature<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(f)
    }
}

#[repr(C)]
struct SystemDescriptorTableHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

pub fn init_tables(rsdp_request: &RsdpRequest) {
    let rsdp_response = rsdp_request
        .get_response()
        .expect("bootloader did not respond to RSDP request");
    debug!("ACPI RSDP address: {:#X}", rsdp_response.address());

    // Safety: Bootloader guarantees root system descriptor pointer is valid.
    let rsdp = unsafe { rsdp::RootSystemDescriptorPointer::from_address(rsdp_response.address()) };

    if !rsdp.is_checksum_valid() {
        error!("ACPI RSDP checksum failed validation.");
    }

    debug!("{rsdp:?}");

    todo!()
}
