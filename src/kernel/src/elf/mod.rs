mod sections;
mod segments;

pub use sections::*;
pub use segments::*;

use libkernel::{Address, Virtual};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
    Other(u8),
}

impl Endianness {
    const fn from_u8(value: u8) -> Self {
        match value {
            0x1 => Self::Little,
            0x2 => Self::Big,
            other => Self::Other(other),
        }
    }
}

#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Abi {
    SystemV,
    HP_UX,
    NetBSD,
    Linux,
    GNU_Hard,
    Solaris,
    AIX,
    IRIX,
    FreeBSD,
    Tru64,
    Novell_Modesto,
    OpenBSD,
    OpenVMS,
    NonStop_Kernel,
    AROS,
    Fenix_OS,
    CloudABI,
    Status_Technologies_OpenVOS,
    Other(u8),
}

impl Abi {
    const fn from_u8(value: u8) -> Self {
        match value {
            0x0 => Self::SystemV,
            0x1 => Self::HP_UX,
            0x2 => Self::NetBSD,
            0x3 => Self::Linux,
            0x4 => Self::GNU_Hard,
            0x5 => Self::Solaris,
            0x7 => Self::AIX,
            0x8 => Self::IRIX,
            0x9 => Self::FreeBSD,
            0xA => Self::Tru64,
            0xB => Self::Novell_Modesto,
            0xC => Self::OpenBSD,
            0xD => Self::OpenVMS,
            0xE => Self::NonStop_Kernel,
            0xF => Self::AROS,
            0x10 => Self::Fenix_OS,
            0x11 => Self::CloudABI,
            0x12 => Self::Status_Technologies_OpenVOS,
            other => Self::Other(other),
        }
    }
}

#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Type {
    Unknown,
    Relocatable,
    Executable,
    Shared,
    Core,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Type {
    const fn from_u16(value: u16) -> Self {
        match value {
            0x0 => Self::Unknown,
            0x1 => Self::Relocatable,
            0x2 => Self::Executable,
            0x3 => Self::Shared,
            0x4 => Self::Core,
            0xFE00..0xFEFF => Self::OsSpecific(value as u8),
            0xFF00..0xFFFF => Self::ProcessorSpecific(value as u8),
            _ => unreachable!(),
        }
    }
}

#[repr(u16)]
#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Machine {
    None,
    ATT_WE_32100,
    SPARC,
    x86,
    Motorola_68000,
    Motorola_88000,
    Intel_MCU,
    Intel_80860,
    MIPS,
    IBM_System_370,
    MIPS_RS3000_LE,
    HP_PA_RISC,
    Intel_80960,
    PowerPC_32bit,
    PowerPC_64bit,
    S390,
    IBM_SP_U_C,
    NEC_V800,
    Fujitsu_FR20,
    TRW_RH_32,
    Motorola_RCE,
    Aarch32,
    DigitalAlpha,
    SuperH,
    SPARC9,
    Siemens_TriCore,
    Argonaut_RISC,
    Hitachi_H8_300,
    Hitachi_H8_300H,
    Hitachi_H8S,
    Hitachi_H8_500,
    IA_64,
    Stanford_MIPS_X,
    Motorola_ColdFire,
    Motorola_M68HC12,
    Fujitsu_MMA,
    Siemens_PCP,
    Sony_nCPU_RISC,
    Denso_NDR1,
    Motorola_StarCore,
    Toyota_ME16,
    STMicroelectronics_ST100,
    AdvancedLogicCorp_TinyJ,
    x86_64,
    Sony_DSP,
    DigitalEquipmentCorp_PDP10,
    DigitalEquipmentCorp_PDP11,
    Siemens_FX66,
    STMicroelectronics_ST9_Plus,
    STMicroelectronics_ST7,
    Motorola_MC68HC16,
    Motorola_MC68HC11,
    Motorola_MC68HC08,
    Motorola_MC68HC05,
    SiliconGraphics_SVx,
    STMicroelectronics_ST19,
    DigitalVAX,
    AxisCommunications_32bit,
    InfineonTechnologies_32bit,
    Element14_DSP_64bit,
    LSILogic_DSP_16bit,
    TMS320C6000_Family,
    MCST_Elbrus_e2k,
    Aarch64,
    ZilogZ80,
    RISC_V,
    BerkeleyPacketFilter,
    WDC_65C816,
    Other(u16),
}

impl Machine {
    const fn from_u16(value: u16) -> Self {
        match value {
            0x00 => Self::None,
            0x01 => Self::ATT_WE_32100,
            0x02 => Self::SPARC,
            0x03 => Self::x86,
            0x04 => Self::Motorola_68000,
            0x05 => Self::Motorola_88000,
            0x06 => Self::Intel_MCU,
            0x07 => Self::Intel_80860,
            0x08 => Self::MIPS,
            0x09 => Self::IBM_System_370,
            0x0A => Self::MIPS_RS3000_LE,
            0x0E => Self::HP_PA_RISC,
            0x13 => Self::Intel_80960,
            0x14 => Self::PowerPC_32bit,
            0x15 => Self::PowerPC_64bit,
            0x16 => Self::S390,
            0x17 => Self::IBM_SP_U_C,
            0x24 => Self::NEC_V800,
            0x25 => Self::Fujitsu_FR20,
            0x26 => Self::TRW_RH_32,
            0x27 => Self::Motorola_RCE,
            0x28 => Self::Aarch32,
            0x29 => Self::DigitalAlpha,
            0x2A => Self::SuperH,
            0x2B => Self::SPARC9,
            0x2C => Self::Siemens_TriCore,
            0x2D => Self::Argonaut_RISC,
            0x2E => Self::Hitachi_H8_300,
            0x2F => Self::Hitachi_H8_300H,
            0x30 => Self::Hitachi_H8S,
            0x31 => Self::Hitachi_H8_500,
            0x32 => Self::IA_64,
            0x33 => Self::Stanford_MIPS_X,
            0x34 => Self::Motorola_ColdFire,
            0x35 => Self::Motorola_M68HC12,
            0x36 => Self::Fujitsu_MMA,
            0x37 => Self::Siemens_PCP,
            0x38 => Self::Sony_nCPU_RISC,
            0x39 => Self::Denso_NDR1,
            0x3A => Self::Motorola_StarCore,
            0x3B => Self::Toyota_ME16,
            0x3C => Self::STMicroelectronics_ST100,
            0x3D => Self::AdvancedLogicCorp_TinyJ,
            0x3E => Self::x86_64,
            0x3F => Self::Sony_DSP,
            0x40 => Self::DigitalEquipmentCorp_PDP10,
            0x41 => Self::DigitalEquipmentCorp_PDP11,
            0x42 => Self::Siemens_FX66,
            0x43 => Self::STMicroelectronics_ST9_Plus,
            0x44 => Self::STMicroelectronics_ST7,
            0x45 => Self::Motorola_MC68HC16,
            0x46 => Self::Motorola_MC68HC11,
            0x47 => Self::Motorola_MC68HC08,
            0x48 => Self::Motorola_MC68HC05,
            0x49 => Self::SiliconGraphics_SVx,
            0x4A => Self::STMicroelectronics_ST19,
            0x4B => Self::DigitalVAX,
            0x4C => Self::AxisCommunications_32bit,
            0x4D => Self::InfineonTechnologies_32bit,
            0x4E => Self::Element14_DSP_64bit,
            0x4F => Self::LSILogic_DSP_16bit,
            0x8C => Self::TMS320C6000_Family,
            0xAF => Self::MCST_Elbrus_e2k,
            0xB7 => Self::Aarch64,
            0xDC => Self::ZilogZ80,
            0xF3 => Self::RISC_V,
            0xF7 => Self::BerkeleyPacketFilter,
            0x101 => Self::WDC_65C816,
            other => Self::Other(other),
        }
    }
}

pub const ELF64_HEADER_SIZE: usize = 64;

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Elf<'a>(&'a [u8]);
// magic: [u8; 4],
// class: Class,
// endianness: Endianness,
// version0: u8,
// abi: Abi,
// abi_version: u8,
// padding: [u8; 7],
// ty: u16,
// machine: Machine,
// version1: u32,
// entry: usize,
// phoff: usize,
// shoff: usize,
// flags: u32,
// ehsize: u16,
// phentsize: u16,
// phcnt: u16,
// shentsize: u16,
// shcnt: u16,
// shstrndx: u16,

impl<'a> Elf<'a> {
    pub const MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

    pub fn from_bytes(bytes: &'a [u8]) -> Option<Self> {
        assert!(bytes.len() > ELF64_HEADER_SIZE, "byte slice is less than the ELF64 header size");

        if Self::MAGIC == bytes[..0x4] && bytes[0x4] == 2 {
            Some(Self(bytes))
        } else {
            None
        }
    }

    pub const fn abi(&self) -> (Abi, u8) {
        (
            match self.0[0x7] {
                0x0 => Abi::SystemV,
                0x1 => Abi::HP_UX,
                0x2 => Abi::NetBSD,
                0x3 => Abi::Linux,
                0x4 => Abi::GNU_Hard,
                0x5 => Abi::Solaris,
                0x7 => Abi::AIX,
                0x8 => Abi::IRIX,
                0x9 => Abi::FreeBSD,
                0xA => Abi::Tru64,
                0xB => Abi::Novell_Modesto,
                0xC => Abi::OpenBSD,
                0xD => Abi::OpenVMS,
                0xE => Abi::NonStop_Kernel,
                0xF => Abi::AROS,
                0x10 => Abi::Fenix_OS,
                0x11 => Abi::CloudABI,
                0x12 => Abi::Status_Technologies_OpenVOS,
                other => Abi::Other(other),
            },
            self.0[0x8],
        )
    }

    #[inline]
    pub fn get_type(&self) -> Type {
        Type::from_u16(u16::from_ne_bytes(self.0[0x10..0x12].try_into().unwrap()))
    }

    #[inline]
    pub fn get_machine(&self) -> Machine {
        Machine::from_u16(u16::from_ne_bytes(self.0[0x12..0x14].try_into().unwrap()))
    }

    #[inline]
    pub fn get_entry_offset(&self) -> u64 {
        u64::from_ne_bytes(self.0[0x18..0x20].try_into().unwrap())
    }

    #[inline]
    pub fn get_segments_offset(&self) -> u64 {
        u64::from_ne_bytes(self.0[0x20..0x28].try_into().unwrap())
    }

    #[inline]
    pub fn get_sections_offset(&self) -> u64 {
        u64::from_ne_bytes(self.0[0x28..0x30].try_into().unwrap())
    }

    #[inline]
    pub fn get_flags(&self) -> u32 {
        u32::from_ne_bytes(self.0[0x30..0x34].try_into().unwrap())
    }

    #[inline]
    pub fn get_segment_header_size(&self) -> u16 {
        u16::from_ne_bytes(self.0[0x36..0x38].try_into().unwrap())
    }

    #[inline]
    pub fn get_segment_headers_count(&self) -> u16 {
        u16::from_ne_bytes(self.0[0x38..0x3A].try_into().unwrap())
    }

    #[inline]
    pub fn get_section_header_size(&self) -> u16 {
        u16::from_ne_bytes(self.0[0x3A..0x3C].try_into().unwrap())
    }

    #[inline]
    pub fn get_section_headers_count(&self) -> u16 {
        u16::from_ne_bytes(self.0[0x3C..0x3E].try_into().unwrap())
    }

    #[inline]
    pub fn get_section_names_header_index(&self) -> u16 {
        u16::from_ne_bytes(self.0[0x3E..0x40].try_into().unwrap())
    }
}

impl core::fmt::Debug for Elf<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("ELF")
            // .field("ELF Header Version", &self.version())
            .field("File Type", &self.get_type())
            .field("ABI / Version", &self.abi())
            .field("Target Machine", &self.get_machine())
            .field("Flags", &self.get_flags())
            .field("Entry Point", &self.get_entry_offset())
            .field("Segment Headers Offset", &self.get_segments_offset())
            .field("Segment Headers Count", &self.get_segment_headers_count())
            .field("Segment Header Size", &self.get_segment_header_size())
            .field("Section Headers Offset", &self.get_sections_offset())
            .field("Section Headers Count", &self.get_section_headers_count())
            .field("Section Header Size", &self.get_section_header_size())
            .field("Section Names Header Index", &self.get_section_names_header_index())
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
