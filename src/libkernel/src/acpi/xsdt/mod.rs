pub mod hpet;
pub mod madt;
pub mod mcfg;

use crate::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};

#[derive(Debug)]
pub enum XSDTError {
    NoXSDT,
    TableNotFound,
}

pub trait SubTable {
    const SIGNATURE: &'static str;

    fn sdt_header(&self) -> &SDTHeader {
        unsafe { (self as *const _ as *const SDTHeader).as_ref().unwrap() }
    }
}

impl<T: SubTable> Checksum for T {
    fn bytes_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

impl<T: SubTable> ACPITable for T {
    fn body_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

#[repr(C)]
pub struct XSDT {
    header: SDTHeader,
}

impl XSDT {
    pub fn header(&self) -> &SDTHeader {
        &self.header
    }

    pub fn find_sub_table<'entry, T: SubTable>(&'entry self) -> Option<&'entry T> {
        for entry_ptr in self.entries() {
            unsafe {
                if (**entry_ptr).signature() == T::SIGNATURE {
                    let table: &T = (*entry_ptr as *const T).as_ref().unwrap();
                    table.validate_checksum();
                    return Some(table);
                }
            }
        }

        None
    }

    pub fn list_sub_tables(&self) {
        debug!("XSDT Sub-Tables:");
        for entry_ptr in self.entries() {
            debug!("{}", unsafe { (**entry_ptr).signature() });
        }
    }
}

impl SizedACPITable<SDTHeader, *const SDTHeader> for XSDT {}

impl ACPITable for XSDT {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl Checksum for XSDT {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
    }
}

pub fn get_xsdt() -> &'static XSDT {
    let xsdt = unsafe { (crate::acpi::get_rsdp().xsdt_addr().as_usize() as *const XSDT).as_ref().unwrap() };
    xsdt.validate_checksum();
    xsdt
}
