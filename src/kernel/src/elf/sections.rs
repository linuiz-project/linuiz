#![allow(non_camel_case_types)]

use libkernel::{Address, Virtual};

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum SectionType {
    NULL = 0x0,
    PROGBITS = 0x1,
    SYMTAB = 0x2,
    STRTAB = 0x3,
    RELA = 0x4,
    HASH = 0x5,
    DYNAMIC = 0x6,
    NOTE = 0x7,
    NOBITS = 0x8,
    REL = 0x9,
    SHLIB = 0xA,
    DYNSYM = 0xB,
    INIT_ARRAY = 0xE,
    FINI_ARRAY = 0xF,
    PREINIT_ARRAY = 0x10,
    GROUP = 0x11,
    SYMTAB_SHNDX = 0x12,
    NUM = 0x13,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct SectionAttributes : usize {
        const WRITE = 1 << 0;
        const ALLOC = 1 << 1;
        const EXECINSTR = 1 << 2;
        const MERGE = 1 << 4;
        const STRINGS = 1 << 5;
        const INFO_LINK = 1 << 6;
        const LINK_ORDER = 1 << 7;
        const OS_NONCONFORMING = 1 << 8;
        const GROUP = 1 << 9;
        const TLS = 1 << 10;
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SectionHeader {
    pub shstrtab_offset: u32,
    pub ty: SectionType,
    pub attribs: SectionAttributes,
    pub addr: Address<Virtual>,
    pub offset: usize,
    pub size: usize,
    pub assc_idx: u32,
    pub info: u32,
    pub addr_align: usize,
    pub entry_size: usize,
}
