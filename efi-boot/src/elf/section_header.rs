#[repr(u32)]
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SectionHeaderType {
    SHT_NULL = 0x0,
    SHT_PROGBITS = 0x1,
    SHT_SYMTAB = 0x2,
    SHT_STRTAB = 0x3,
    SHT_RELA = 0x4,
    SHT_HASH = 0x5,
    SHT_DYNAMIC = 0x6,
    SHT_NOTE = 0x7,
    SHT_NOBITS = 0x8,
    SHT_REL = 0x9,
    SHT_SHLIB = 0xA,
    SHT_DYNSYM = 0xB,
    SHT_INIT_ARRAY = 0xE,
    SHT_FINI_ARRAY = 0xF,
    SHT_PREINIT_ARRAY = 0x10,
    SHT_GROUP = 0x11,
    SHT_SYMTAB_SHNDX = 0x12,
    SHT_NUM = 0x13,
    SHT_LOOS = 0x60000000,
}

#[repr(usize)]
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SectionHeaderFlags {
    SHF_WRITE = 0x1,
    SHF_ALLOC = 0x2,
    SHF_EXECINSTR = 0x4,
    SHF_MERGE = 0x10,
    SHF_STRINGS = 0x20,
    SHF_INFO_LINK = 0x40,
    SHF_LINK_ORDER = 0x80,
    SHF_OS_NONCONFORMING = 0x100,
    SHF_GROUP = 0x200,
    SHF_TLS = 0x400,
    SHF_MASKOS = 0xFF00000,
    SHF_MASKPROC = 0xF0000000,
    SHF_ORDERED = 0x4000000,
    SHF_EXCLUDE = 0x8000000,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SectionHeader {
    name: u32,
    sh_type: SectionHeaderType,
    flags: SectionHeaderFlags,
    addr: usize,
    offset: usize,
    size: usize,
    link: u32,
    info: u32,
    addralign: usize,
    section_size: usize,
}

impl SectionHeader {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        // verify length of passed slice
        if bytes.len() < core::mem::size_of::<SectionHeader>() {
            None
        } else {
            unsafe {
                let header_ptr = bytes.as_ptr() as *const SectionHeader;
                // this version of the header relies on the buffer data, which is unsafe
                let temp_header = *header_ptr;
                // so we return a clone
                Some(temp_header.clone())
            }
        }
    }

    pub fn name(&self) -> u32 {
        self.name
    }

    pub fn sh_type(&self) -> SectionHeaderType {
        self.sh_type
    }

    pub fn flags(&self) -> SectionHeaderFlags {
        self.flags
    }

    pub fn address(&self) -> usize {
        self.addr
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn link(&self) -> u32 {
        self.link
    }

    pub fn info(&self) -> u32 {
        self.info
    }

    pub fn address_align(&self) -> usize {
        self.addralign
    }

    pub fn section_size(&self) -> usize {
        self.section_size
    }
}

impl core::fmt::Debug for SectionHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        formatter
            .debug_struct("Section Header")
            .field("Name", &self.name)
            .field("Type", &self.sh_type)
            .field("Flags", &self.flags)
            .field("Address", &self.addr)
            .field("Offset", &self.offset)
            .field("Size", &self.size)
            .field("Link", &self.link)
            .field("Info", &self.info)
            .field("Address Alignment", &self.addralign)
            .field("Section Size", &self.section_size)
            .finish()
    }
}
