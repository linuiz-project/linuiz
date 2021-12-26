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
        unsafe { &*(self as *const _ as *const _) }
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
pub struct XSDTData {
    header: SDTHeader,
}

impl XSDTData {
    pub fn header(&self) -> &SDTHeader {
        &self.header
    }

    pub fn find_sub_table<'entry, T: SubTable>(&'entry self) -> Result<&'entry T, XSDTError> {
        for entry_ptr in self.entries() {
            unsafe {
                if (**entry_ptr).signature() == T::SIGNATURE {
                    let table: &T = &*(*entry_ptr as *const _);
                    table.validate_checksum();
                    return Ok(table);
                }
            }
        }

        Err(XSDTError::TableNotFound)
    }
}

impl SizedACPITable<SDTHeader, *const SDTHeader> for XSDTData {}

impl ACPITable for XSDTData {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl Checksum for XSDTData {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
    }
}

lazy_static::lazy_static! {
    pub static ref XSDT: &'static XSDTData = unsafe {
            let xsdt = &*(crate::acpi::rdsp::LAZY_RDSP2.xsdt_addr().as_usize() as *const XSDTData);
            xsdt.validate_checksum();
            xsdt
    };
}
