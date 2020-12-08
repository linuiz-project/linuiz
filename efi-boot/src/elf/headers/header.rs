#[repr(u8)]
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ELFABI {
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
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ELFType {
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
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ELFMachine {
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

#[repr(C)]
#[allow(unused_imports, non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ELFHeader64 {
    magic: [u8; 4],
    class: u8,
    endianness: u8,
    hversion: u8,
    abi: ELFABI,
    abi_version: u8,
    padding: [u8; 7],
    elf_type: ELFType,
    machine: ELFMachine,
    eversion: u32,
    entry: usize,
    phoff: usize,
    shoff: usize,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}

impl ELFHeader64 {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        // verify length of passed slice
        if bytes.len() < core::mem::size_of::<ELFHeader64>() {
            None
        } else {
            unsafe {
                let header_ptr = bytes.as_ptr() as *const ELFHeader64;
                // this version of the header relies on the buffer data, which is unsafe
                let temp_header = *header_ptr;
                // so we return a clone
                Some(temp_header.clone())
            }
        }
    }

    pub fn entry_address(&self) -> usize {
        self.entry
    }

    pub fn header_size(&self) -> u16 {
        self.ehsize
    }

    pub fn program_header_size(&self) -> u16 {
        self.phentsize
    }

    pub fn program_header_offset(&self) -> usize {
        self.phoff
    }

    pub fn program_header_count(&self) -> u16 {
        self.phnum
    }

    pub fn section_header_size(&self) -> u16 {
        self.shentsize
    }

    pub fn section_header_offset(&self) -> usize {
        self.phoff
    }

    pub fn section_header_count(&self) -> u16 {
        self.shnum
    }
}

impl core::fmt::Debug for ELFHeader64 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        formatter
            .debug_struct("ELF")
            .field("Class", &self.class)
            .field("Endianness", &self.endianness)
            .field("ELF Header Version", &self.hversion)
            .field("ABI", &self.abi)
            .field("ABI Version", &self.abi_version)
            .field("File Type", &self.elf_type)
            .field("Target Machine", &self.machine)
            .field("ELF Version", &self.eversion)
            .field("Entry Point", &self.entry)
            .field("Program Header Offset", &self.phoff)
            .field("Section Header Offset", &self.shoff)
            .field("Flags", &self.flags)
            .field("ELF Header Size", &self.ehsize)
            .field("Program Header Size", &self.phentsize)
            .field("Program Header Number", &self.phnum)
            .field("Section Header Size", &self.shentsize)
            .field("Section Header Number", &self.shnum)
            .field("Section Header Start Index", &self.shstrndx)
            .finish()
    }
}
