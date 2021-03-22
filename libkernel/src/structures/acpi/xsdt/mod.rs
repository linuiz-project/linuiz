mod madt_entry;
mod mcfg_entry;

pub use madt_entry::*;
pub use mcfg_entry::*;

use crate::structures::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};

#[repr(C)]
pub struct XSDT {
    header: SDTHeader,
}

impl XSDT {
    pub fn header(&self) -> &SDTHeader {
        &self.header
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

pub trait XSDTEntryType {
    const SIGNATURE: &'static str;
}

pub struct XSDTEntry<T: XSDTEntryType> {
    phantom: core::marker::PhantomData<T>,
}

impl<T: XSDTEntryType> XSDTEntry<T> {
    pub fn sdt_header(&self) -> &SDTHeader {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<T: XSDTEntryType> Checksum for XSDTEntry<T> {
    fn bytes_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

impl<T: XSDTEntryType> ACPITable for XSDTEntry<T> {
    fn body_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

#[derive(Debug)]
pub enum XSDTError {
    NotInitialized,
    NoEntry,
}

pub fn get_entry<T: XSDTEntryType>() -> Result<&'static XSDTEntry<T>, XSDTError> {
    if G_XSDT.is_some() {
        for entry_ptr in G_XSDT.unwrap().entries().iter().map(|entry_ptr| *entry_ptr) {
            unsafe {
                if (&*(entry_ptr as *const _ as *const SDTHeader)).signature() == T::SIGNATURE {
                    return Ok(&*(entry_ptr as *const _ as *const _));
                }
            }
        }

        Err(XSDTError::NoEntry)
    } else {
        Err(XSDTError::NotInitialized)
    }
}

lazy_static::lazy_static! {
    static ref G_XSDT: Option<&'static XSDT> = unsafe {
        crate::structures::acpi::G_RDSP2.map(|rdsp2| &*(rdsp2.xsdt_addr().as_usize() as *const XSDT))
    };
}

// TODO xsdt subtables should be derived from a base struct, with T variants deriving them
//      this will allow returning arbitrary tables from a function, where T is the variant
//      (..... ideally)
