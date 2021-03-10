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

impl crate::structures::acpi::Checksum for RDSPDescriptor {}

#[repr(C, packed)]
pub struct RDSPDescriptor2 {
    base: RDSPDescriptor,
    len: u32,
    xsdt_addr: crate::Address<crate::addr_ty::Physical>,
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

    pub fn xsdt(&self) -> &crate::structures::acpi::XSDT {
        unsafe { &*(self.xsdt_addr.as_usize() as *const crate::structures::acpi::XSDT) }
    }
}

impl crate::structures::acpi::Checksum for RDSPDescriptor2 {}
