use crate::structures::acpi::{Checksum, SDTHeader};

#[repr(C, packed)]
pub struct XSDT {
    header: SDTHeader,
    sdt_ptr_head: *const u64,
}

impl XSDT {
    fn sdt_ptrs_len(&self) -> usize {
        ((unsafe { self.header.len() as usize }) - core::mem::size_of::<SDTHeader>()) / 8
    }

    pub fn sdt_ptrs(&self) -> &[*const u64] {
        unsafe { &*core::ptr::slice_from_raw_parts(&self.sdt_ptr_head, self.sdt_ptrs_len()) }
    }

    pub fn index_as<T>(&self, index: usize) -> &T {
        unsafe { &*(self.sdt_ptrs()[index] as *const T) }
    }
}

impl Checksum for XSDT {
    fn bytes_len(&self) -> usize {
        unsafe { self.header.len() as usize }
    }
}
