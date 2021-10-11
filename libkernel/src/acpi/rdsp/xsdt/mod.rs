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
pub struct XSDT<'entry> {
    header: SDTHeader,
    phantom: core::marker::PhantomData<&'entry core::ffi::c_void>,
}

impl<'entry> XSDT<'entry> {
    pub fn header(&self) -> &SDTHeader {
        &self.header
    }

    pub fn find_sub_table<T: SubTable>(&self) -> Result<&'entry T, XSDTError> {
        for entry_ptr in self.entries().iter() {
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

impl SizedACPITable<SDTHeader, *const SDTHeader> for XSDT<'_> {}

impl ACPITable for XSDT<'_> {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl Checksum for XSDT<'_> {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
    }
}

lazy_static::lazy_static! {
    pub static ref LAZY_XSDT: &'static XSDT<'static> = unsafe {
            let xsdt = &*(crate::acpi::rdsp::LAZY_RDSP2.xsdt_addr().as_usize() as *const XSDT<'static>);
            xsdt.validate_checksum();
            xsdt
    };
}
