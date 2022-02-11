use crate::instructions::cpuid::exec;
use bitflags::bitflags;
use core::fmt::{Debug, Display, Formatter, Result};
use lazy_static::lazy_static;

bitflags! {
    pub struct Features : u64 {
        const SSE3          = 1 << 0;
        const PCLMULQDQ     = 1 << 1;
        const DTES64        = 1 << 2;
        const MONITOR       = 1 << 3;
        const DS_CPL        = 1 << 4;
        const VMX           = 1 << 5;
        const SMX           = 1 << 6;
        const EIST          = 1 << 7;
        const TM2           = 1 << 8;
        const SSSE3         = 1 << 9;
        const CNXTID        = 1 << 10;
        const SDBG          = 1 << 11;
        const FMA           = 1 << 12;
        const CMPXG16       = 1 << 13;
        const XTPR          = 1 << 14;
        const PDCM          = 1 << 15;
        const PCID          = 1 << 17;
        const DCA           = 1 << 18;
        const SSE41         = 1 << 19;
        const SSE42         = 1 << 20;
        const X2APIC        = 1 << 21;
        const MOVBE         = 1 << 22;
        const POPCNT        = 1 << 23;
        const TSC_DL        = 1 << 24;
        const AESNI         = 1 << 25;
        const XSAVE         = 1 << 26;
        const OSXSAVE       = 1 << 27;
        const AVX           = 1 << 28;
        const F16C          = 1 << 29;
        const RDRAND        = 1 << 30;
        const FPU           = 1 << 32;
        const VME           = 1 << 33;
        const DE            = 1 << 34;
        const PSE           = 1 << 35;
        const TSC           = 1 << 36;
        const MSR           = 1 << 37;
        const PAE           = 1 << 38;
        const MCE           = 1 << 39;
        const CX8           = 1 << 40;
        const APIC          = 1 << 41;
        const SEP           = 1 << 43;
        const MTRR          = 1 << 44;
        const PGE           = 1 << 45;
        const MCA           = 1 << 46;
        const CMOV          = 1 << 47;
        const PAT           = 1 << 48;
        const PSE36         = 1 << 49;
        const PSN           = 1 << 50;
        const CLFSH         = 1 << 51;
        const DSTR          = 1 << 53;
        const ACPI          = 1 << 54;
        const MMX           = 1 << 55;
        const FXSR          = 1 << 56;
        const SSE           = 1 << 57;
        const SSE2          = 1 << 58;
        const SS            = 1 << 59;
        const HTT           = 1 << 60;
        const TM            = 1 << 61;
        const PBE           = 1 << 63;
    }
}

impl Debug for FEATURES {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        Features::fmt(self, formatter)
    }
}

lazy_static! {
    pub static ref FEATURES: Features = {
        let cpuid = exec(0x1, 0x0).unwrap();
        Features::from_bits_truncate(((cpuid.edx() as u64) << 32) | (cpuid.ecx() as u64))
    };
}

bitflags! {
    pub struct FeaturesExt : u64 {
        const LAHF          = 1 << 0;
        const LZCNT         = 1 << 5;
        const PREFETCHW     = 1 << 8;
        const SYSCALL       = 1 << 43;
        const NO_EXEC       = 1 << 52;
        const GB_PAGES      = 1 << 58;
        const RDTSCP        = 1 << 59;
        const IA_64         = 1 << 61;
    }
}

lazy_static! {
    pub static ref FEATURES_EXT: FeaturesExt = {
        let cpuid = exec(0x80000001, 0x0).unwrap();
        FeaturesExt::from_bits_truncate(((cpuid.edx() as u64) << 32) | (cpuid.ecx() as u64))
    };
}

impl Debug for FEATURES_EXT {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        FeaturesExt::fmt(self, formatter)
    }
}

lazy_static! {
    pub static ref VENDOR: [u8; 12] = {
        let result = exec(0x0, 0x0).unwrap();
        let mut bytes = [0u8; 12];

        bytes[0..4].copy_from_slice(&result.ebx().to_le_bytes());
        bytes[4..8].copy_from_slice(&result.edx().to_le_bytes());
        bytes[8..12].copy_from_slice(&result.ecx().to_le_bytes());

        bytes
    };
}

impl Debug for VENDOR {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        <&str as Debug>::fmt(
            unsafe { &core::str::from_utf8_unchecked(&**self) },
            formatter,
        )
    }
}

impl Display for VENDOR {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        <&str as Display>::fmt(
            unsafe { &core::str::from_utf8_unchecked(&**self) },
            formatter,
        )
    }
}

#[no_mangle]
pub unsafe fn ring3_enter(target_func: fn(), rflags: crate::registers::RFlags) {
    core::arch::asm!(
    "sysretq",
    in("rcx") target_func,
    in("r11") rflags.bits(),
    options(noreturn)
    );
}

pub fn is_bsp() -> bool {
    crate::registers::msr::IA32_APIC_BASE::is_bsp()
}

/// Enumerates the most-recent available CPUID leaf for the core ID.
pub fn get_id() -> u32 {
    if let Some(registers) =
        // IA32 SDM instructs to enumerate this leaf first...
        exec(0x1F, 0x0)
            // ... this leaf second ...
            .or_else(|| exec(0xB, 0x0))
    {
        registers.edx()
    } else if let Some(registers) =
        // ... and finally, this leaf as an absolute fallback.
        exec(0x1, 0x0)
    {
        registers.ebx() >> 24
    } else {
        panic!("CPUID ID enumeration failed.");
    }
}
