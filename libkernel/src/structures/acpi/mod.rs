mod madt;
mod rdsp;
mod xsdt;

pub use madt::*;
pub use rdsp::*;
pub use xsdt::*;

pub trait Checksum: Sized {
    fn bytes_len(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn checksum(&self) -> bool {
        let mut sum: u8 = 0;

        unsafe {
            &*core::ptr::slice_from_raw_parts(
                core::mem::transmute::<&Self, *const u8>(self),
                self.bytes_len(),
            )
        }
        .iter()
        .for_each(|byte| sum = sum.wrapping_add(*byte));

        sum == 0
    }
}

#[repr(C)]
pub struct SDTHeader {
    signature: [u8; 4],
    len: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

impl SDTHeader {
    pub fn signature(&self) -> &str {
        core::str::from_utf8(&self.signature).expect("invalid ascii sequence for signature")
    }

    pub const fn len(&self) -> u32 {
        self.len
    }

    pub const fn revision(&self) -> u8 {
        self.revision
    }

    pub fn oem_id(&self) -> &str {
        core::str::from_utf8(&self.oem_id).expect("invalid ascii sequence for OEM ID")
    }

    pub fn oem_table_id(&self) -> &str {
        core::str::from_utf8(&self.oem_table_id).expect("invalid ascii sequence for OEM table ID")
    }
}

impl Checksum for SDTHeader {}

impl core::fmt::Debug for SDTHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("System Description Table Header")
            .field("Signature", &self.signature())
            .field("OEM ID", &self.oem_id())
            .field("Revision", &self.revision())
            .finish()
    }
}
