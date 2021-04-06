pub mod madt;
pub mod mcfg;

use crate::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};

#[derive(Debug)]
pub enum XSDTError {
    NoXSDT,
    TableNotFound,
}

pub trait XSDTSubTableType {
    const SIGNATURE: &'static str;
}

pub struct XSDTSubTable<T: XSDTSubTableType> {
    phantom: core::marker::PhantomData<T>,
}

impl<T: XSDTSubTableType> XSDTSubTable<T> {
    pub fn sdt_header(&self) -> &SDTHeader {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<T: XSDTSubTableType> Checksum for XSDTSubTable<T> {
    fn bytes_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

impl<T: XSDTSubTableType> ACPITable for XSDTSubTable<T> {
    fn body_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

#[repr(C)]
pub struct XSDT<'entry> {
    header: SDTHeader,
    phantom: core::marker::PhantomData<&'entry u8>,
}

impl<'entry> XSDT<'entry> {
    pub fn header(&self) -> &SDTHeader {
        &self.header
    }

    pub fn find_sub_table<T: XSDTSubTableType>(
        &self,
    ) -> Result<&'entry XSDTSubTable<T>, XSDTError> {
        for entry_ptr in self.entries().iter().map(|entry_ptr| *entry_ptr) {
            unsafe {
                if (&*(entry_ptr as *const _ as *const SDTHeader)).signature() == T::SIGNATURE {
                    let table: &XSDTSubTable<T> = &*(entry_ptr as *const _ as *const _);
                    table.checksum_panic();
                    return Ok(table);
                }
            }
        }

        Err(XSDTError::TableNotFound)
    }
}

impl ACPITable for XSDT<'_> {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl SizedACPITable<SDTHeader, *const u64> for XSDT<'_> {}

impl Checksum for XSDT<'_> {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
    }
}

lazy_static::lazy_static! {
    pub static ref LAZY_XSDT: Option<&'static XSDT<'static>> = unsafe {
        crate::acpi::rdsp::LAZY_RDSP2.map(|rdsp2| {
            let xsdt = &*(rdsp2.xsdt_addr().as_usize() as *const XSDT<'static>);
            xsdt.checksum_panic();
            xsdt
        })
    };
}
