#[inline]
pub fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);

    unsafe {
        asm!(
            "xchg rsi, rbx",
            "cpuid",
            "xchg rsi, rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            lateout("esi") ebx,
            lateout("edx") edx,
        )
    }

    (eax, ebx, ecx, edx)
}

bitflags::bitflags! {
    pub struct CPUFeatures: u64 {
        const SSE3         = 1 << 0;
        const PCLMUL       = 1 << 1;
        const DTES64       = 1 << 2;
        const MONITOR      = 1 << 3;
        const DS_CPL       = 1 << 4;
        const VMX          = 1 << 5;
        const SMX          = 1 << 6;
        const EST          = 1 << 7;
        const TM2          = 1 << 8;
        const SSSE3        = 1 << 9;
        const CID          = 1 << 10;
        const FMA          = 1 << 12;
        const CX16         = 1 << 13;
        const ETPRD        = 1 << 14;
        const PDCM         = 1 << 15;
        const PCIDE        = 1 << 17;
        const DCA          = 1 << 18;
        const SSE4_1       = 1 << 19;
        const SSE4_2       = 1 << 20;
        const X2APIC       = 1 << 21;
        const MOVBE        = 1 << 22;
        const POPCNT       = 1 << 23;
        const AES          = 1 << 25;
        const XSAVE        = 1 << 26;
        const OSXSAVE      = 1 << 27;
        const AVX          = 1 << 28;
        const FPU          = 1 << 32;
        const VME          = 1 << 33;
        const DE           = 1 << 34;
        const PSE          = 1 << 35;
        const TSC          = 1 << 36;
        const MSR          = 1 << 37;
        const PAE          = 1 << 38;
        const MCE          = 1 << 39;
        const CX8          = 1 << 40;
        const APIC         = 1 << 41;
        const SEP          = 1 << 43;
        const MTRR         = 1 << 44;
        const PGE          = 1 << 45;
        const MCA          = 1 << 46;
        const CMOV         = 1 << 47;
        const PAT          = 1 << 48;
        const PSE36        = 1 << 49;
        const PSN          = 1 << 50;
        const CLF          = 1 << 51;
        const DTES         = 1 << 53;
        const ACPI         = 1 << 54;
        const MMX          = 1 << 55;
        const FXSR         = 1 << 56;
        const SSE          = 1 << 57;
        const SSE2         = 1 << 58;
        const SS           = 1 << 59;
        const HTT          = 1 << 60;
        const TM1          = 1 << 61;
        const IA64         = 1 << 62;
        const PBE          = 1 << 63;
    }
}

#[inline]
pub fn cpu_features() -> CPUFeatures {
    let values = cpuid(0x1, 0x0);
    CPUFeatures::from_bits_truncate(((values.3 as u64) << 32) | (values.2 as u64))
}
