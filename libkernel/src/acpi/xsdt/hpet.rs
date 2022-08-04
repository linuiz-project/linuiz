use crate::acpi::xsdt;
use core::fmt::{Debug, Formatter, Result};

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AddressData {
    // 0 = system memory, 1 = system I/O
    pub addr_space_id: u8,
    pub register_bit_width: u8,
    pub register_bit_offset: u8,
    resv0: u8,
    pub address: crate::Address<crate::Physical>,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Header {
    sdt_header: xsdt::SDTHeader,
    hw_rev_id: u8,
    bits0: u8,
    pci_vendor_id: u16,
    address: AddressData,
    hpet_num: u8,
    minimum_tick: u16,
    page_prot: u8,
}

pub struct HPET;

impl xsdt::SubTable for HPET {
    const SIGNATURE: &'static str = &"HPET";
}

impl HPET {
    fn hept_header(&self) -> Header {
        unsafe { *(self as *const _ as *const Header).as_ref().unwrap() }
    }

    pub fn min_tick(&self) -> u16 {
        let header = self.hept_header();
        header.minimum_tick
    }

    pub fn address_data(&self) -> AddressData {
        let address = self.hept_header().address;
        address
    }
}

impl Debug for HPET {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_struct("HPET")
            // TODO
            .finish()
    }
}
