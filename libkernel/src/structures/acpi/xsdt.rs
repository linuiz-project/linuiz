use crate::structures::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable, MADT, MCFG};

#[repr(C)]
pub struct XSDT {
    header: SDTHeader,
}

impl XSDT {
    pub fn header(&self) -> SDTHeader {
        self.header
    }

    pub fn iter(&self) -> XSDTEntryIterator {
        XSDTEntryIterator {
            entries: self.entries(),
            index: 0,
        }
    }
}

impl ACPITable for XSDT {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl SizedACPITable<SDTHeader, *const u64> for XSDT {}

impl Checksum for XSDT {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
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
                    "MCFG" => Some(XSDTEntry::MCFG(&*(entry_ptr as *const MCFG))),
                    ident => Some(XSDTEntry::NotSupported(ident)),
                }
            }
        } else {
            None
        }
    }
}

pub enum XSDTEntry<'a> {
    APIC(&'a MADT),
    MCFG(&'a MCFG),
    NotSupported(&'a str),
}
