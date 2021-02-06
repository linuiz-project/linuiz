use x86_64::PhysAddr;

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

    pub fn checksum(&self) -> bool {
        self.sum() == 0
    }

    fn sum(&self) -> u8 {
        let mut sum: u8 = 0;

        unsafe {
            &*core::ptr::slice_from_raw_parts(
                core::mem::transmute::<&Self, *const u8>(self),
                core::mem::size_of::<Self>(),
            )
        }
        .iter()
        .for_each(|byte| sum = sum.wrapping_add(*byte));

        sum
    }
}

#[repr(C, packed)]
pub struct RDSPDescriptor2 {
    base: RDSPDescriptor,
    len: u32,
    xsdt_addr: PhysAddr,
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

    pub fn checksum(&self) -> bool {
        self.sum() == 0
    }

    fn sum(&self) -> u8 {
        let mut sum: u8 = 0;

        unsafe {
            &*core::ptr::slice_from_raw_parts(
                core::mem::transmute::<&Self, *const u8>(self),
                core::mem::size_of::<Self>() - self.reserved.len(),
            )
        }
        .iter()
        .for_each(|byte| sum = sum.wrapping_add(*byte));

        sum
    }
}
