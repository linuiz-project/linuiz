use crate::structures::acpi::{Checksum, SDTHeader, MADT};

#[repr(C, packed)]
pub struct XSDT {
    header: SDTHeader,
    sdt_head_ptr: *const u64,
}

impl XSDT {
    fn body_len(&self) -> usize {
        (unsafe { self.header.len() as usize }) - core::mem::size_of::<SDTHeader>()
    }

    pub fn iter(&self) -> XSDTEntryIterator {
        XSDTEntryIterator {
            entries: unsafe {
                &*core::ptr::slice_from_raw_parts(
                    &self.sdt_head_ptr,
                    self.body_len() / core::mem::size_of::<*const u64>(),
                )
            },
            index: 0,
        }
    }
}

impl Checksum for XSDT {
    fn bytes_len(&self) -> usize {
        unsafe { self.header.len() as usize }
    }
}

pub struct XSDTEntryIterator<'entries> {
    entries: &'entries [*const u64],
    index: usize,
}

impl<'entries> Iterator for XSDTEntryIterator<'entries> {
    type Item = XSDTEntry<'entries>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.entries.len() {
            unsafe {
                let entry_ptr = self.entries[self.index];
                self.index += 1;

                match core::str::from_utf8(&*(entry_ptr as *const [u8; 4])).unwrap() {
                    "APIC" => Some(XSDTEntry::APIC(&*(entry_ptr as *const MADT))),
                    _ => Some(XSDTEntry::NotSupported),
                }
            }
        } else {
            None
        }
    }
}

pub enum XSDTEntry<'a> {
    APIC(&'a MADT),
    NotSupported,
}
