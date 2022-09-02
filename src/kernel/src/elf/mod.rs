mod section_header;
mod segment_header;

pub use section_header::*;
pub use segment_header::*;

use libkernel::{Address, Virtual};

use num_enum::TryFromPrimitive;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Bit32 = 1,
    Bit64 = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little = 1,
    Big = 2,
}

#[repr(u8)]
#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Abi {
    SystemV = 0x0,
    HP_UX = 0x1,
    NetBSD = 0x2,
    Linux = 0x3,
    GNU_Hard = 0x4,
    Solaris = 0x5,
    AIX = 0x7,
    IRIX = 0x8,
    FreeBSD = 0x9,
    Tru64 = 0xA,
    Novell_Modesto = 0xB,
    OpenBSD = 0xC,
    OpenVMS = 0xD,
    NonStop_Kernel = 0xE,
    AROS = 0xF,
    Fenix_OS = 0x10,
    CloudABI = 0x11,
    Status_Technologies_OpenVOS = 0x12,
}

#[repr(u16)]
#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
pub enum Type {
    ET_NONE = 0x0,
    ET_REL = 0x1,
    ET_EXEC = 0x2,
    ET_DYN = 0x3,
    ET_CORE = 0x4,
    ET_LOOS = 0xFE00,
    ET_HIOS = 0xFEFF,
    ET_LOPROC = 0xFF00,
    ET_HIPROC = 0xFFFF,
}

#[repr(u16)]
#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
pub enum Machine {
    None = 0x0,
    ATT_WE_32100 = 0x1,
    SPARC = 0x2,
    x86 = 0x3,
    Moto_68000_M68k = 0x4,
    Moto_68000_M88k = 0x5,
    Intel_MCU = 0x6,
    Intel_80860 = 0x7,
    MIPS = 0x8,
    IBM_System370 = 0x9,
    MIPS_RS3000_LEndi = 0xA,
    HP_PA_RISC = 0xE,
    Intel_80960 = 0x13,
    PowerPC32 = 0x14,
    PowerPC64 = 0x15,
    S390 = 0x16,
    ARM32 = 0x28,
    SuperH = 0x2A,
    IA_64 = 0x32,
    AMD64 = 0x3E,
    TMS320C6000_Family = 0x8C,
    ARM64 = 0xB7,
    RISC_V = 0xF3,
    WDC_65C816 = 0x101,
}

#[repr(C, packed)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Elf {
    magic: [u8; 4],
    class: Class,
    endianness: Endianness,
    version0: u8,
    abi: Abi,
    abi_version: u8,
    padding: [u8; 7],
    ty: [u8; 2],
    machine: [u8; 2],
    version1: u32,
    entry: usize,
    phoff: usize,
    shoff: usize,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phcnt: u16,
    shentsize: u16,
    shcnt: u16,
    shstrndx: u16,
}

impl Elf {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // verify length of passed slice
        if bytes.len() < core::mem::size_of::<Elf>() {
            None
        } else {
            unsafe {
                const HEADER_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

                let header_ptr = bytes.as_ptr() as *const Elf;
                // this version of the header relies on the buffer data, which is unsafe
                let temp_header = *header_ptr;

                // verify the header's magic number
                if !temp_header.magic.iter().zip(HEADER_MAGIC.iter()).all(|(a, b)| a == b) {
                    None
                } else {
                    // so we return a clone
                    Some(temp_header.clone())
                }
            }
        }
    }

    // TODO add getters for all properties

    pub const fn class(&self) -> Class {
        self.class
    }

    pub const fn endianness(&self) -> Endianness {
        self.endianness
    }

    pub const fn abi(&self) -> (Abi, u8) {
        (self.abi, self.abi_version)
    }

    pub fn get_type(&self) -> Type {
        Type::try_from(match self.endianness() {
            Endianness::Little => u16::from_le_bytes(self.ty),
            Endianness::Big => u16::from_be_bytes(self.ty),
        })
        .unwrap()
    }

    pub fn get_machine(&self) -> Machine {
        Machine::try_from(match self.endianness() {
            Endianness::Little => u16::from_le_bytes(self.machine),
            Endianness::Big => u16::from_be_bytes(self.machine),
        })
        .unwrap()
    }

    pub fn entry_address(&self) -> usize {
        self.entry
    }

    pub fn header_size(&self) -> u16 {
        self.ehsize
    }

    pub fn segment_header_size(&self) -> u16 {
        self.phentsize
    }

    pub fn segment_headers_offset(&self) -> usize {
        self.phoff
    }

    pub fn segment_header_count(&self) -> u16 {
        self.phcnt
    }

    pub fn section_header_size(&self) -> u16 {
        self.shentsize
    }

    pub fn section_headers_offset(&self) -> usize {
        self.shoff
    }

    pub fn section_header_count(&self) -> u16 {
        self.shcnt
    }

    /// Contains index of the section header table entry that contains the section names.
    pub fn section_header_string_index(&self) -> u16 {
        self.shstrndx
    }
}

impl core::fmt::Debug for Elf {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("ELF")
            .field("Class", &self.class())
            .field("Endianness", &self.endianness())
            // .field("ELF Header Version", &self.version())
            .field("ABI / Version", &self.abi())
            .field("File Type", &self.get_type())
            .field("Target Machine", &self.get_machine())
            // .field("ELF Version", &self.eversion())
            // .field("Entry Point", &self.entry())
            // .field("Program Header Offset", &self.phoff())
            // .field("Section Header Offset", &self.shoff())
            // .field("Flags", &self.flags())
            // .field("ELF Header Size", &self.ehsize())
            // .field("Program Header Size", &self.phentsize())
            // .field("Program Header Count", &self.phcnt())
            // .field("Section Header Size", &self.shentsize())
            // .field("Section Header Count", &self.shcnt())
            // .field("Section Header String Index", &self.shstrndx())
            .finish()
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum RelaType {
    X86_NONE = 0x0,
    x86_PC32 = 0x1,
    x86_32 = 0x2,
    X86_GOT32 = 0x3,
    X86_PLT32 = 0x4,
    X86_COPY = 0x5,
    X86_GLOB_DAT = 0x6,
    X86_JMP_SLOT = 0x7,
    X86_RELATIVE = 0x8,
    X86_GOTOFF = 0x9,
    X86_GOTPC = 0xA,
    X86_32PLT = 0xB,
    X86_16 = 0x14,
    X86_PC16 = 0x15,
    X86_8 = 0x16,
    X86_PC8 = 0x17,
    X86_SIZE32 = 0x18,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Rela64 {
    pub addr: Address<Virtual>,
    pub ty: RelaType,
    pub sym_idx: u32,
    pub addend: u64,
}
