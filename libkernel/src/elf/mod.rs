mod header;
mod section_header;
mod segment_header;

pub use header::*;
pub use section_header::*;
pub use segment_header::*;

use libarch::{Address, Virtual};

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Rela64 {
    pub addr: Address<Virtual>,
    pub ty: RelaType,
    pub sym_idx: u32,
    pub addend: u64,
}
