use crate::instructions::cpuid::exec;
use bitflags::bitflags;
use core::fmt::{Debug, Display, Formatter, Result};
use lazy_static::lazy_static;

const FEATURES_01H: usize = 0 << 16;
const FEATURES_07H: usize = 1 << 16;
const FEATURES_80000001H: usize = 2 << 16;

const EAX: usize = 0 << 24;
const EBX: usize = 1 << 24;
const ECX: usize = 2 << 24;
const EDX: usize = 3 << 24;

lazy_static::lazy_static! {
    static ref FEATURES1: [[u32; 4]; 3] = {
        let features_01h_regs = exec(0x1, 0x0).map_or([0u32; 4], |regs| {
            [regs.eax(), regs.ebx(), regs.ecx(), regs.edx()]
        });

        let features_07h_regs = exec(0x7, 0x0).map_or([0u32; 4], |regs| {
            [regs.eax(), regs.ebx(), regs.ecx(), regs.edx()]
        });

        let features_80000001h_regs = exec(0x80000001, 0x0).map_or([0u32; 4], |regs| {
            [regs.eax(), regs.ebx(), regs.ecx(), regs.edx()]
        });

        [features_01h_regs, features_07h_regs, features_80000001h_regs]
    };
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[repr(usize)]
#[derive(Debug)]
pub enum Feature {
    // 01H.ECX
    SSE3                = FEATURES_01H | ECX | 0,
    PCLMULQDQ           = FEATURES_01H | ECX | 1,
    DTES64              = FEATURES_01H | ECX | 2,
    MONITOR             = FEATURES_01H | ECX | 3,
    DS_CPL              = FEATURES_01H | ECX | 4,
    VMX                 = FEATURES_01H | ECX | 5,
    SMX                 = FEATURES_01H | ECX | 6,
    EIST                = FEATURES_01H | ECX | 7,
    TM2                 = FEATURES_01H | ECX | 8,
    SSSE3               = FEATURES_01H | ECX | 9,
    CNXTID              = FEATURES_01H | ECX | 10,
    SDBG                = FEATURES_01H | ECX | 11,
    FMA                 = FEATURES_01H | ECX | 12,
    CMPXG16             = FEATURES_01H | ECX | 13,
    XTPR                = FEATURES_01H | ECX | 14,
    PDCM                = FEATURES_01H | ECX | 15,
    PCID                = FEATURES_01H | ECX | 17,
    DCA                 = FEATURES_01H | ECX | 18,
    SSE41               = FEATURES_01H | ECX | 19,
    SSE42               = FEATURES_01H | ECX | 20,
    X2APIC              = FEATURES_01H | ECX | 21,
    MOVBE               = FEATURES_01H | ECX | 22,
    POPCNT              = FEATURES_01H | ECX | 23,
    TSC_DL              = FEATURES_01H | ECX | 24,
    AESNI               = FEATURES_01H | ECX | 25,
    XSAVE               = FEATURES_01H | ECX | 26,
    OSXSAVE             = FEATURES_01H | ECX | 27,
    AVX                 = FEATURES_01H | ECX | 28,
    F16C                = FEATURES_01H | ECX | 29,
    RDRAND              = FEATURES_01H | ECX | 30,
    // 01H.EDX
    FPU                 = FEATURES_01H | EDX | 0,
    VME                 = FEATURES_01H | EDX | 1,
    DE                  = FEATURES_01H | EDX | 2,
    PSE                 = FEATURES_01H | EDX | 3,
    TSC                 = FEATURES_01H | EDX | 4,
    MSR                 = FEATURES_01H | EDX | 5,
    PAE                 = FEATURES_01H | EDX | 6,
    MCE                 = FEATURES_01H | EDX | 7,
    CX8                 = FEATURES_01H | EDX | 8,
    APIC                = FEATURES_01H | EDX | 9,
    SEP                 = FEATURES_01H | EDX | 11,
    MTRR                = FEATURES_01H | EDX | 12,
    PGE                 = FEATURES_01H | EDX | 13,
    MCA                 = FEATURES_01H | EDX | 14,
    CMOV                = FEATURES_01H | EDX | 15,
    PAT                 = FEATURES_01H | EDX | 16,
    PSE36               = FEATURES_01H | EDX | 17,
    PSN                 = FEATURES_01H | EDX | 18,
    CLFSH               = FEATURES_01H | EDX | 19,
    DSTR                = FEATURES_01H | EDX | 21,
    ACPI                = FEATURES_01H | EDX | 22,
    MMX                 = FEATURES_01H | EDX | 23,
    FXSR                = FEATURES_01H | EDX | 24,
    SSE                 = FEATURES_01H | EDX | 25,
    SSE2                = FEATURES_01H | EDX | 26,
    SS                  = FEATURES_01H | EDX | 27,
    HTT                 = FEATURES_01H | EDX | 28,
    TM                  = FEATURES_01H | EDX | 29,
    PBE                 = FEATURES_01H | EDX | 31,

    // 07H.EBX
    FSGSBASE            = FEATURES_07H | EBX | 0,
    TSC_ADJUST          = FEATURES_07H | EBX | 1,
    SGX                 = FEATURES_07H | EBX | 2,
    BMI1                = FEATURES_07H | EBX | 3,
    HLE                 = FEATURES_07H | EBX | 4,
    AVX2                = FEATURES_07H | EBX | 5,
    FDP_EXCEP_ONLY      = FEATURES_07H | EBX | 6,
    SMEP                = FEATURES_07H | EBX | 7,
    BMI2                = FEATURES_07H | EBX | 8,
    ENH_REP_MOV_STO_SB  = FEATURES_07H | EBX | 9,
    INVPCID             = FEATURES_07H | EBX | 10,
    RTM                 = FEATURES_07H | EBX | 11,
    RDT_M               = FEATURES_07H | EBX | 12,
    DPR_FPU_CS_DS       = FEATURES_07H | EBX | 13,
    MPX                 = FEATURES_07H | EBX | 14,
    RDT_A               = FEATURES_07H | EBX | 15,
    AVX512F             = FEATURES_07H | EBX | 16,
    AVX512DQ            = FEATURES_07H | EBX | 17,
    RDSEED              = FEATURES_07H | EBX | 18,
    ADX                 = FEATURES_07H | EBX | 19,
    SMAP                = FEATURES_07H | EBX | 20,
    AVX512_IFMA         = FEATURES_07H | EBX | 21,
    CLFLUSHPOT          = FEATURES_07H | EBX | 23,
    CLWB                = FEATURES_07H | EBX | 24,
    PROC_TRACE          = FEATURES_07H | EBX | 25,
    ACV512PF            = FEATURES_07H | EBX | 26,
    AVX512ER            = FEATURES_07H | EBX | 27,
    AVX512CD            = FEATURES_07H | EBX | 28,
    SHA                 = FEATURES_07H | EBX | 29,
    AVX512BW            = FEATURES_07H | EBX | 30,
    AVX512VL            = FEATURES_07H | EBX | 31,
    // 07H.ECX
    PREFETCHWT1         = FEATURES_07H | ECX | 0,
    AVX512_VBMI         = FEATURES_07H | ECX | 1,
    UMIP                = FEATURES_07H | ECX | 2,
    PKU                 = FEATURES_07H | ECX | 3,
    OSPKE               = FEATURES_07H | ECX | 4,
    WAITPKG             = FEATURES_07H | ECX | 5,
    AVX512_VBMI2        = FEATURES_07H | ECX | 6,
    CET_SS              = FEATURES_07H | ECX | 7,
    GFNI                = FEATURES_07H | ECX | 8,
    VAES                = FEATURES_07H | ECX | 9,
    CPVLMULQDQ          = FEATURES_07H | ECX | 10,
    AVX512_VNNI         = FEATURES_07H | ECX | 11,
    AVX512_BITALG       = FEATURES_07H | ECX | 12,
    TME_EN              = FEATURES_07H | ECX | 13,
    AVX512_CPOPCNTDQ    = FEATURES_07H | ECX | 14,
    LA57                = FEATURES_07H | ECX | 16,
    // 17..=21 : The value of MAWAU used by the BNDLDX and BNDSTX instructions in 64-bit mode.
    RPID                = FEATURES_07H | ECX | 22,
    KL                  = FEATURES_07H | ECX | 23,
    CLDEMOTE            = FEATURES_07H | ECX | 25,
    MOVDIRI             = FEATURES_07H | ECX | 27,
    MOVDIRI64B          = FEATURES_07H | ECX | 28,
    SGX_LC              = FEATURES_07H | ECX | 30,
    PKS                 = FEATURES_07H | ECX | 31,
    // 07H.EDX
    AVX512_4VNNIW       = FEATURES_07H | EDX | 2,
    AVX512_4FMAPS       = FEATURES_07H | EDX | 3,
    FAST_SHORT_REP_MOV  = FEATURES_07H | EDX | 4,
    AVX512_VP2INTERSECT = FEATURES_07H | EDX | 8,
    MD_CLEAR            = FEATURES_07H | EDX | 10,
    HYBRID              = FEATURES_07H | EDX | 15,
    PCONFIG             = FEATURES_07H | EDX | 18,
    CET_IBT             = FEATURES_07H | EDX | 20,
    IBRS_IBPB           = FEATURES_07H | EDX | 26,
    STIBP               = FEATURES_07H | EDX | 27,
    L1D_FLUSH           = FEATURES_07H | EDX | 28,
    ARCH_CAP            = FEATURES_07H | EDX | 29,
    CORE_CAP            = FEATURES_07H | EDX | 30,
    SSBD                = FEATURES_07H | EDX | 31,

    // 80000001H.ECX
    L_S_AHF             = FEATURES_80000001H | ECX | 0,
    LZCNT               = FEATURES_80000001H | ECX | 5,
    PREFETCHW           = FEATURES_80000001H | ECX | 8,
    // 80000001H.EDX
    SYSCALL             = FEATURES_80000001H | EDX | 11,
    NXE                 = FEATURES_80000001H | EDX | 20,
    GB_PAGES            = FEATURES_80000001H | EDX | 26,
    RDTSC               = FEATURES_80000001H | EDX | 27,
    IA_64               = FEATURES_80000001H | EDX | 29,
}

pub struct FeatureFmt;

impl core::fmt::Debug for FeatureFmt {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut cpu_features_fmt = formatter.debug_list();

        if has_feature(Feature::SSE3) {
            cpu_features_fmt.entry(&Feature::SSE3);
        }
        if has_feature(Feature::PCLMULQDQ) {
            cpu_features_fmt.entry(&Feature::PCLMULQDQ);
        }
        if has_feature(Feature::DTES64) {
            cpu_features_fmt.entry(&Feature::DTES64);
        }
        if has_feature(Feature::MONITOR) {
            cpu_features_fmt.entry(&Feature::MONITOR);
        }
        if has_feature(Feature::DS_CPL) {
            cpu_features_fmt.entry(&Feature::DS_CPL);
        }
        if has_feature(Feature::VMX) {
            cpu_features_fmt.entry(&Feature::VMX);
        }
        if has_feature(Feature::SMX) {
            cpu_features_fmt.entry(&Feature::SMX);
        }
        if has_feature(Feature::EIST) {
            cpu_features_fmt.entry(&Feature::EIST);
        }
        if has_feature(Feature::TM2) {
            cpu_features_fmt.entry(&Feature::TM2);
        }
        if has_feature(Feature::SSSE3) {
            cpu_features_fmt.entry(&Feature::SSSE3);
        }
        if has_feature(Feature::CNXTID) {
            cpu_features_fmt.entry(&Feature::CNXTID);
        }
        if has_feature(Feature::SDBG) {
            cpu_features_fmt.entry(&Feature::SDBG);
        }
        if has_feature(Feature::FMA) {
            cpu_features_fmt.entry(&Feature::FMA);
        }
        if has_feature(Feature::CMPXG16) {
            cpu_features_fmt.entry(&Feature::CMPXG16);
        }
        if has_feature(Feature::XTPR) {
            cpu_features_fmt.entry(&Feature::XTPR);
        }
        if has_feature(Feature::PDCM) {
            cpu_features_fmt.entry(&Feature::PDCM);
        }
        if has_feature(Feature::PCID) {
            cpu_features_fmt.entry(&Feature::PCID);
        }
        if has_feature(Feature::DCA) {
            cpu_features_fmt.entry(&Feature::DCA);
        }
        if has_feature(Feature::SSE41) {
            cpu_features_fmt.entry(&Feature::SSE41);
        }
        if has_feature(Feature::SSE42) {
            cpu_features_fmt.entry(&Feature::SSE42);
        }
        if has_feature(Feature::X2APIC) {
            cpu_features_fmt.entry(&Feature::X2APIC);
        }
        if has_feature(Feature::MOVBE) {
            cpu_features_fmt.entry(&Feature::MOVBE);
        }
        if has_feature(Feature::POPCNT) {
            cpu_features_fmt.entry(&Feature::POPCNT);
        }
        if has_feature(Feature::TSC_DL) {
            cpu_features_fmt.entry(&Feature::TSC_DL);
        }
        if has_feature(Feature::AESNI) {
            cpu_features_fmt.entry(&Feature::AESNI);
        }
        if has_feature(Feature::XSAVE) {
            cpu_features_fmt.entry(&Feature::XSAVE);
        }
        if has_feature(Feature::OSXSAVE) {
            cpu_features_fmt.entry(&Feature::OSXSAVE);
        }
        if has_feature(Feature::AVX) {
            cpu_features_fmt.entry(&Feature::AVX);
        }
        if has_feature(Feature::F16C) {
            cpu_features_fmt.entry(&Feature::F16C);
        }
        if has_feature(Feature::RDRAND) {
            cpu_features_fmt.entry(&Feature::RDRAND);
        }
        if has_feature(Feature::FPU) {
            cpu_features_fmt.entry(&Feature::FPU);
        }
        if has_feature(Feature::VME) {
            cpu_features_fmt.entry(&Feature::VME);
        }
        if has_feature(Feature::DE) {
            cpu_features_fmt.entry(&Feature::DE);
        }
        if has_feature(Feature::PSE) {
            cpu_features_fmt.entry(&Feature::PSE);
        }
        if has_feature(Feature::TSC) {
            cpu_features_fmt.entry(&Feature::TSC);
        }
        if has_feature(Feature::MSR) {
            cpu_features_fmt.entry(&Feature::MSR);
        }
        if has_feature(Feature::PAE) {
            cpu_features_fmt.entry(&Feature::PAE);
        }
        if has_feature(Feature::MCE) {
            cpu_features_fmt.entry(&Feature::MCE);
        }
        if has_feature(Feature::CX8) {
            cpu_features_fmt.entry(&Feature::CX8);
        }
        if has_feature(Feature::APIC) {
            cpu_features_fmt.entry(&Feature::APIC);
        }
        if has_feature(Feature::SEP) {
            cpu_features_fmt.entry(&Feature::SEP);
        }
        if has_feature(Feature::MTRR) {
            cpu_features_fmt.entry(&Feature::MTRR);
        }
        if has_feature(Feature::PGE) {
            cpu_features_fmt.entry(&Feature::PGE);
        }
        if has_feature(Feature::MCA) {
            cpu_features_fmt.entry(&Feature::MCA);
        }
        if has_feature(Feature::CMOV) {
            cpu_features_fmt.entry(&Feature::CMOV);
        }
        if has_feature(Feature::PAT) {
            cpu_features_fmt.entry(&Feature::PAT);
        }
        if has_feature(Feature::PSE36) {
            cpu_features_fmt.entry(&Feature::PSE36);
        }
        if has_feature(Feature::PSN) {
            cpu_features_fmt.entry(&Feature::PSN);
        }
        if has_feature(Feature::CLFSH) {
            cpu_features_fmt.entry(&Feature::CLFSH);
        }
        if has_feature(Feature::DSTR) {
            cpu_features_fmt.entry(&Feature::DSTR);
        }
        if has_feature(Feature::ACPI) {
            cpu_features_fmt.entry(&Feature::ACPI);
        }
        if has_feature(Feature::MMX) {
            cpu_features_fmt.entry(&Feature::MMX);
        }
        if has_feature(Feature::FXSR) {
            cpu_features_fmt.entry(&Feature::FXSR);
        }
        if has_feature(Feature::SSE) {
            cpu_features_fmt.entry(&Feature::SSE);
        }
        if has_feature(Feature::SSE2) {
            cpu_features_fmt.entry(&Feature::SSE2);
        }
        if has_feature(Feature::SS) {
            cpu_features_fmt.entry(&Feature::SS);
        }
        if has_feature(Feature::HTT) {
            cpu_features_fmt.entry(&Feature::HTT);
        }
        if has_feature(Feature::TM) {
            cpu_features_fmt.entry(&Feature::TM);
        }
        if has_feature(Feature::PBE) {
            cpu_features_fmt.entry(&Feature::PBE);
        }
        if has_feature(Feature::FSGSBASE) {
            cpu_features_fmt.entry(&Feature::FSGSBASE);
        }
        if has_feature(Feature::TSC_ADJUST) {
            cpu_features_fmt.entry(&Feature::TSC_ADJUST);
        }
        if has_feature(Feature::SGX) {
            cpu_features_fmt.entry(&Feature::SGX);
        }
        if has_feature(Feature::BMI1) {
            cpu_features_fmt.entry(&Feature::BMI1);
        }
        if has_feature(Feature::HLE) {
            cpu_features_fmt.entry(&Feature::HLE);
        }
        if has_feature(Feature::AVX2) {
            cpu_features_fmt.entry(&Feature::AVX2);
        }
        if has_feature(Feature::FDP_EXCEP_ONLY) {
            cpu_features_fmt.entry(&Feature::FDP_EXCEP_ONLY);
        }
        if has_feature(Feature::SMEP) {
            cpu_features_fmt.entry(&Feature::SMEP);
        }
        if has_feature(Feature::BMI2) {
            cpu_features_fmt.entry(&Feature::BMI2);
        }
        if has_feature(Feature::ENH_REP_MOV_STO_SB) {
            cpu_features_fmt.entry(&Feature::ENH_REP_MOV_STO_SB);
        }
        if has_feature(Feature::INVPCID) {
            cpu_features_fmt.entry(&Feature::INVPCID);
        }
        if has_feature(Feature::RTM) {
            cpu_features_fmt.entry(&Feature::RTM);
        }
        if has_feature(Feature::RDT_M) {
            cpu_features_fmt.entry(&Feature::RDT_M);
        }
        if has_feature(Feature::DPR_FPU_CS_DS) {
            cpu_features_fmt.entry(&Feature::DPR_FPU_CS_DS);
        }
        if has_feature(Feature::MPX) {
            cpu_features_fmt.entry(&Feature::MPX);
        }
        if has_feature(Feature::RDT_A) {
            cpu_features_fmt.entry(&Feature::RDT_A);
        }
        if has_feature(Feature::AVX512F) {
            cpu_features_fmt.entry(&Feature::AVX512F);
        }
        if has_feature(Feature::AVX512DQ) {
            cpu_features_fmt.entry(&Feature::AVX512DQ);
        }
        if has_feature(Feature::RDSEED) {
            cpu_features_fmt.entry(&Feature::RDSEED);
        }
        if has_feature(Feature::ADX) {
            cpu_features_fmt.entry(&Feature::ADX);
        }
        if has_feature(Feature::SMAP) {
            cpu_features_fmt.entry(&Feature::SMAP);
        }
        if has_feature(Feature::AVX512_IFMA) {
            cpu_features_fmt.entry(&Feature::AVX512_IFMA);
        }
        if has_feature(Feature::CLFLUSHPOT) {
            cpu_features_fmt.entry(&Feature::CLFLUSHPOT);
        }
        if has_feature(Feature::CLWB) {
            cpu_features_fmt.entry(&Feature::CLWB);
        }
        if has_feature(Feature::PROC_TRACE) {
            cpu_features_fmt.entry(&Feature::PROC_TRACE);
        }
        if has_feature(Feature::ACV512PF) {
            cpu_features_fmt.entry(&Feature::ACV512PF);
        }
        if has_feature(Feature::AVX512ER) {
            cpu_features_fmt.entry(&Feature::AVX512ER);
        }
        if has_feature(Feature::AVX512CD) {
            cpu_features_fmt.entry(&Feature::AVX512CD);
        }
        if has_feature(Feature::SHA) {
            cpu_features_fmt.entry(&Feature::SHA);
        }
        if has_feature(Feature::AVX512BW) {
            cpu_features_fmt.entry(&Feature::AVX512BW);
        }
        if has_feature(Feature::AVX512VL) {
            cpu_features_fmt.entry(&Feature::AVX512VL);
        }
        if has_feature(Feature::PREFETCHWT1) {
            cpu_features_fmt.entry(&Feature::PREFETCHWT1);
        }
        if has_feature(Feature::AVX512_VBMI) {
            cpu_features_fmt.entry(&Feature::AVX512_VBMI);
        }
        if has_feature(Feature::UMIP) {
            cpu_features_fmt.entry(&Feature::UMIP);
        }
        if has_feature(Feature::PKU) {
            cpu_features_fmt.entry(&Feature::PKU);
        }
        if has_feature(Feature::OSPKE) {
            cpu_features_fmt.entry(&Feature::OSPKE);
        }
        if has_feature(Feature::WAITPKG) {
            cpu_features_fmt.entry(&Feature::WAITPKG);
        }
        if has_feature(Feature::AVX512_VBMI2) {
            cpu_features_fmt.entry(&Feature::AVX512_VBMI2);
        }
        if has_feature(Feature::CET_SS) {
            cpu_features_fmt.entry(&Feature::CET_SS);
        }
        if has_feature(Feature::GFNI) {
            cpu_features_fmt.entry(&Feature::GFNI);
        }
        if has_feature(Feature::VAES) {
            cpu_features_fmt.entry(&Feature::VAES);
        }
        if has_feature(Feature::CPVLMULQDQ) {
            cpu_features_fmt.entry(&Feature::CPVLMULQDQ);
        }
        if has_feature(Feature::AVX512_VNNI) {
            cpu_features_fmt.entry(&Feature::AVX512_VNNI);
        }
        if has_feature(Feature::AVX512_BITALG) {
            cpu_features_fmt.entry(&Feature::AVX512_BITALG);
        }
        if has_feature(Feature::TME_EN) {
            cpu_features_fmt.entry(&Feature::TME_EN);
        }
        if has_feature(Feature::AVX512_CPOPCNTDQ) {
            cpu_features_fmt.entry(&Feature::AVX512_CPOPCNTDQ);
        }
        if has_feature(Feature::LA57) {
            cpu_features_fmt.entry(&Feature::LA57);
        }
        if has_feature(Feature::RPID) {
            cpu_features_fmt.entry(&Feature::RPID);
        }
        if has_feature(Feature::KL) {
            cpu_features_fmt.entry(&Feature::KL);
        }
        if has_feature(Feature::CLDEMOTE) {
            cpu_features_fmt.entry(&Feature::CLDEMOTE);
        }
        if has_feature(Feature::MOVDIRI) {
            cpu_features_fmt.entry(&Feature::MOVDIRI);
        }
        if has_feature(Feature::MOVDIRI64B) {
            cpu_features_fmt.entry(&Feature::MOVDIRI64B);
        }
        if has_feature(Feature::SGX_LC) {
            cpu_features_fmt.entry(&Feature::SGX_LC);
        }
        if has_feature(Feature::PKS) {
            cpu_features_fmt.entry(&Feature::PKS);
        }
        if has_feature(Feature::AVX512_4VNNIW) {
            cpu_features_fmt.entry(&Feature::AVX512_4VNNIW);
        }
        if has_feature(Feature::AVX512_4FMAPS) {
            cpu_features_fmt.entry(&Feature::AVX512_4FMAPS);
        }
        if has_feature(Feature::FAST_SHORT_REP_MOV) {
            cpu_features_fmt.entry(&Feature::FAST_SHORT_REP_MOV);
        }
        if has_feature(Feature::AVX512_VP2INTERSECT) {
            cpu_features_fmt.entry(&Feature::AVX512_VP2INTERSECT);
        }
        if has_feature(Feature::MD_CLEAR) {
            cpu_features_fmt.entry(&Feature::MD_CLEAR);
        }
        if has_feature(Feature::HYBRID) {
            cpu_features_fmt.entry(&Feature::HYBRID);
        }
        if has_feature(Feature::PCONFIG) {
            cpu_features_fmt.entry(&Feature::PCONFIG);
        }
        if has_feature(Feature::CET_IBT) {
            cpu_features_fmt.entry(&Feature::CET_IBT);
        }
        if has_feature(Feature::IBRS_IBPB) {
            cpu_features_fmt.entry(&Feature::IBRS_IBPB);
        }
        if has_feature(Feature::STIBP) {
            cpu_features_fmt.entry(&Feature::STIBP);
        }
        if has_feature(Feature::L1D_FLUSH) {
            cpu_features_fmt.entry(&Feature::L1D_FLUSH);
        }
        if has_feature(Feature::ARCH_CAP) {
            cpu_features_fmt.entry(&Feature::ARCH_CAP);
        }
        if has_feature(Feature::CORE_CAP) {
            cpu_features_fmt.entry(&Feature::CORE_CAP);
        }
        if has_feature(Feature::SSBD) {
            cpu_features_fmt.entry(&Feature::SSBD);
        }
        if has_feature(Feature::L_S_AHF) {
            cpu_features_fmt.entry(&Feature::L_S_AHF);
        }
        if has_feature(Feature::LZCNT) {
            cpu_features_fmt.entry(&Feature::LZCNT);
        }
        if has_feature(Feature::PREFETCHW) {
            cpu_features_fmt.entry(&Feature::PREFETCHW);
        }
        if has_feature(Feature::SYSCALL) {
            cpu_features_fmt.entry(&Feature::SYSCALL);
        }
        if has_feature(Feature::NXE) {
            cpu_features_fmt.entry(&Feature::NXE);
        }
        if has_feature(Feature::GB_PAGES) {
            cpu_features_fmt.entry(&Feature::GB_PAGES);
        }
        if has_feature(Feature::RDTSC) {
            cpu_features_fmt.entry(&Feature::RDTSC);
        }
        if has_feature(Feature::IA_64) {
            cpu_features_fmt.entry(&Feature::IA_64);
        }

        cpu_features_fmt.finish()
    }
}

pub fn has_feature(feature: Feature) -> bool {
    use bit_field::BitField;

    let feature_usize = feature as usize;
    let feature_type = (feature_usize & (0xFF << 16)) >> 16;
    let feature_reg = (feature_usize & (0xFF << 24)) >> 24;

    FEATURES1[feature_type][feature_reg].get_bit(feature_usize & 0xFFFF)
}

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
