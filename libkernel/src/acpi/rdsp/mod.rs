pub mod xsdt;

use crate::{acpi::Checksum, addr_ty::Physical, Address};

#[repr(C, packed)]
pub struct RDSPDescriptor {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
}

impl RDSPDescriptor {
    pub fn signature(&self) -> &str {
        core::str::from_utf8(&self.signature).expect("invalid ascii sequence for signature")
    }

    pub fn oem_id(&self) -> &str {
        core::str::from_utf8(&self.oem_id).expect("invalid ascii sequence for OEM id")
    }
}

impl Checksum for RDSPDescriptor {}

#[repr(C, packed)]
pub struct RDSPDescriptor2 {
    base: RDSPDescriptor,
    len: u32,
    xsdt_addr: Address<Physical>,
    ext_checksum: u8,
    reserved: [u8; 3],
}

impl RDSPDescriptor2 {
    pub fn signature(&self) -> &str {
        self.base.signature()
    }

    pub fn oem_id(&self) -> &str {
        self.base.oem_id()
    }

    pub fn xsdt_addr(&self) -> Address<Physical> {
        self.xsdt_addr
    }
}

impl Checksum for RDSPDescriptor2 {}

lazy_static::lazy_static! {
    pub static ref LAZY_RDSP2: Option<&'static RDSPDescriptor2> = unsafe {
        if let Some(entry) = crate::acpi::get_system_config_table_entry(crate::acpi::ACPI2_GUID) {
            Some(entry.as_ref())
        } else {
            None
        }
    };
}
