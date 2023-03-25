#![allow(non_camel_case_types)]

use libsys::{Address, Virtual};

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum Type {
    Unused,
    ProgramData,
    SymbolTable,
    StringTable,
    RelocationAddends,
    SymbolHashTable,
    DynamicLinkingInfo,
    Notes,
    ZeroData,
    Relocations,
    DynamicLinkingSymbolTable,
    ConstructorsArray,
    DestuctorsArray,
    PrecontrusctorsArray,
    SectionGroup,
    ExtendedSectionIndexes,
    DefinedTypeCount,
    OsSpecific(u32),
}

impl Type {
    const fn from_u32(value: u32) -> Self {
        match value {
            0x0 => Self::Unused,
            0x1 => Self::ProgramData,
            0x2 => Self::SymbolTable,
            0x3 => Self::StringTable,
            0x4 => Self::RelocationAddends,
            0x5 => Self::SymbolHashTable,
            0x6 => Self::DynamicLinkingInfo,
            0x7 => Self::Notes,
            0x8 => Self::ZeroData,
            0x9 => Self::Relocations,
            0xB => Self::DynamicLinkingSymbolTable,
            0xE => Self::ConstructorsArray,
            0xF => Self::DestuctorsArray,
            0x10 => Self::PrecontrusctorsArray,
            0x11 => Self::SectionGroup,
            0x12 => Self::ExtendedSectionIndexes,
            0x13 => Self::DefinedTypeCount,
            0x60000000..u32::MAX => Self::OsSpecific(value),
            _ => unimplemented!(),
        }
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    pub struct Attributes : u64 {
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

        // 0x0FF00000 — OS specific
        // 0xF0000000 — Processor specific
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Header {
    shstrtab_offset: u32,
    ty: u32,
    attributes: u64,
    virt_addr: u64,
    file_offset: u64,
    disk_size: u64,
    assc_index: u32,
    extra_info: u32,
    align: u64,
    entry_size: u64,
}

// Safety: Type is composed of simple primitive numerics.
unsafe impl bytemuck::AnyBitPattern for Header {}
// Safety: Type is composed of simple primitive numerics.
unsafe impl bytemuck::Zeroable for Header {}

impl Header {
    #[inline]
    pub const fn get_names_section_offset(&self) -> usize {
        self.shstrtab_offset as usize
    }

    #[inline]
    pub const fn get_type(&self) -> Type {
        Type::from_u32(self.ty)
    }

    #[inline]
    pub const fn get_attributes(&self) -> Attributes {
        Attributes::from_bits_truncate(self.attributes)
    }

    #[inline]
    pub fn get_virtual_address(&self) -> Option<Address<Virtual>> {
        Address::new(self.virt_addr as usize)
    }

    #[inline]
    pub const fn get_file_offset(&self) -> usize {
        self.file_offset as usize
    }

    #[inline]
    pub const fn get_disk_size(&self) -> usize {
        self.disk_size as usize
    }

    #[inline]
    pub const fn get_associated_index(&self) -> usize {
        self.assc_index as usize
    }

    #[inline]
    pub const fn get_extra_info(&self) -> u32 {
        self.extra_info
    }

    #[inline]
    pub fn get_alignment(&self) -> usize {
        self.align as usize
    }
}

pub struct Section<'a> {
    header: &'a Header,
    data: &'a [u8],
}

impl core::ops::Deref for Section<'_> {
    type Target = Header;

    fn deref(&self) -> &Self::Target {
        self.header
    }
}

impl<'a> Section<'a> {
    #[inline]
    pub const fn new(header: &'a Header, data: &'a [u8]) -> Self {
        Self { header, data }
    }

    #[inline]
    pub const fn data(&self) -> &[u8] {
        self.data
    }
}

impl core::fmt::Debug for Section<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("Section")
            .field("Type", &self.get_type())
            .field("Attributes", &self.get_attributes())
            .field("File Offset", &self.get_file_offset())
            .field("Disk Size", &self.get_disk_size())
            .field("Virtual Address", &self.get_virtual_address())
            .field("Alignment", &self.get_alignment())
            .field("Associated Index", &self.get_associated_index())
            .field("Extra Info", &self.get_extra_info())
            .finish()
    }
}
