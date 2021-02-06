mod rdsp;

pub use rdsp::*;

pub trait Checksum: Sized {
    fn checksum(&self) -> Option<()> {
        let mut sum: u8 = 0;

        unsafe {
            &*core::ptr::slice_from_raw_parts(
                core::mem::transmute::<&Self, *const u8>(self),
                core::mem::size_of::<Self>(),
            )
        }
        .iter()
        .for_each(|byte| sum = sum.wrapping_add(*byte));

        match sum {
            0 => Some(()),
            _ => None,
        }
    }
}

#[repr(C, packed)]
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

    pub fn oem_id(&self) -> &str {
        core::str::from_utf8(&self.oem_id).expect("invalid ascii sequence for OEM ID")
    }

    pub fn oem_table_id(&self) -> &str {
        core::str::from_utf8(&self.oem_table_id).expect("invalid ascii sequence for OEM table ID")
    }
}

impl Checksum for SDTHeader {}
