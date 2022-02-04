#![allow(non_camel_case_types)]

//! UNSAFETY: It is *possible* that the current CPU doesn't support the MSR
//!           feature. In this case, well... all of this fails. And we're
//!           going to ignore that. :)

use bit_field::BitField;
use x86_64::registers::segmentation::SegmentSelector;

use crate::{Physical, Address};

#[inline(always)]
fn rdmsr(ecx: u32) -> u64 {
    unsafe {
        let value: u64;
        core::arch:: asm!(
            "push rax",     // Preserve the `rax` value.
            "rdmsr",
            "shl rdx, 32",  // Shift high value to high bits
            "or rdx, rax",  // Copy low value in
            "pop rax",      // Return the preserved `rax` value
            in("ecx") ecx,
            out("rdx") value,
            options(nostack, nomem)
        );
        value
    }
}

#[inline(always)]
unsafe fn wrmsr(ecx: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") ecx,
        in("rax") value,
        in("rdx") value >> 32,
        options(nostack, nomem)
    );
}

pub trait GenericMSR {
    const ECX: u32;

    #[inline(always)]
    fn read() -> u64 {
        rdmsr(Self::ECX)
    }

    #[inline(always)]
    unsafe fn write(value: u64) {
        wrmsr(Self::ECX, value);
    }
}

pub struct IA32_APIC_BASE;
impl IA32_APIC_BASE {
    /// Gets the 8th bit of the IA32_APIC_BASE MSR, which indicates whether the current APIC resides on the boot processor.
    #[inline(always)]
    pub fn is_bsp() -> bool {
        rdmsr(0x1B).get_bit(8)
    }

    /// Gets the 11th bit of the IA32_APIC_BASE MSR, getting the enable state of the APIC.
    #[inline(always)]
    pub fn get_hw_enable() -> bool {
        rdmsr(0x1B).get_bit(11)
    }

    /// Sets the 11th bit of the IA32_APIC_BASE MSR, setting the enable state of the APIC.
    ///
    /// > 1. Using the APIC global enable/disable flag in the IA32_APIC_BASE MSR (MSR address 1BH; see Figure 10-5):
    /// >   — When IA32_APIC_BASE[11] is 0, the processor is functionally equivalent to an IA-32 processor without an
    /// >     on-chip APIC. The CPUID feature flag for the APIC (see Section 10.4.2, “Presence of the Local APIC”) is also
    /// >     set to 0.
    /// >   — When IA32_APIC_BASE[11] is set to 0, processor APICs based on the 3-wire APIC bus cannot be generally
    /// >     re-enabled until a system hardware reset. The 3-wire bus loses track of arbitration that would be necessary
    /// >     for complete re-enabling. Certain APIC functionality can be enabled (for example: performance and
    /// >     thermal monitoring interrupt generation).
    /// >   — For processors that use Front Side Bus (FSB) delivery of interrupts, software may disable or enable the
    /// >     APIC by setting and resetting IA32_APIC_BASE[11]. A hardware reset is not required to re-start APIC
    /// >     functionality, if software guarantees no interrupt will be sent to the APIC as IA32_APIC_BASE[11] is
    /// >     cleared.
    /// >   — When IA32_APIC_BASE[11] is set to 0, prior initialization to the APIC may be lost and the APIC may return
    /// >     to the state described in Section 10.4.7.1, “Local APIC State After Power-Up or Reset.
    #[inline(always)]
    pub fn set_hw_enable(enable: bool) {
        unsafe { wrmsr(0x1B, *rdmsr(0x1B).set_bit(11, enable)) };
    }

    /// Gets bits 12..36 of the IA32_APIC_BASE MSR, representing the base address of the APIC.
    #[inline(always)]
    pub fn get_base_addr() -> Address<Physical> {
        Address::<Physical>::new((rdmsr(0x1B) & 0xFFFFFF000) as usize)
    }
}

pub struct IA32_EFER;
impl IA32_EFER {
    /// Gets the IA32_EFER.SCE (syscall/syret enable) bit.
    #[inline(always)]
    pub fn get_sce() -> bool {
        rdmsr(0xC0000080).get_bit(0)
    }

    /// Sets the IA32_EFER.SCE (syscall/syret enable) bit.
    #[inline(always)]
    pub fn set_sce(set: bool) {
        unsafe { wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(0, set)) };
    }

    /// Gets the IA32_EFER.LMA (long-mode active) bit.
    #[inline(always)]
    pub fn get_lma() -> bool {
        rdmsr(0xC0000080).get_bit(10)
    }

    /// Sets the IA32_EFER.LME (long-mode enable) bit.
    #[inline(always)]
    pub fn set_lme(set: bool) {
        unsafe { wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(8, set)) };
    }

    /// Gets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline(always)]
    pub fn get_nxe() -> bool {
        rdmsr(0xC0000080).get_bit(11)
    }

    /// Sets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline(always)]
    pub fn set_nxe(set: bool) {
        assert!(crate::cpu::FEATURES_EXT.contains(crate::cpu::FeaturesExt::NO_EXEC), "Cannot enable IA32_EFER.NXE if CPU does not support it (CPUID.80000001H:EDX.NX [bit 20]).");

        unsafe { wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(11, set)) };
    }
}

pub struct IA32_STAR;
impl IA32_STAR {
    /// Sets the selectors used for `sysret`.
    ///
    /// Usage (from the IA32 specification):
    /// > When SYSRET transfers control to 64-bit mode user code using REX.W, the processor gets the privilege level 3
    /// > target code segment, instruction pointer, stack segment, and flags as follows:
    /// > Target code segment —             Reads a non-NULL selector from IA32_STAR[63:48] + 16.
    /// > ...
    /// > Stack segment —                   IA32_STAR[63:48] + 8
    /// > ...
    #[inline(always)]
    pub fn set_selectors(low_selector: SegmentSelector, high_selector: SegmentSelector) {
        unsafe {
            wrmsr(
                0xC0000081,
                (high_selector.index() as u64) << 51 | (low_selector.index() as u64) << 35,
            )
        };
    }
}

pub struct IA32_LSTAR;
impl GenericMSR for IA32_LSTAR {
    const ECX: u32 = 0xC0000082;
}

pub struct IA32_CSTAR;
impl GenericMSR for IA32_CSTAR {
    const ECX: u32 = 0xC0000083;
}

pub struct IA32_SFMASK;
impl GenericMSR for IA32_SFMASK {
    const ECX: u32 = 0xC0000084;
}

pub struct IA32_FS_BASE;
impl GenericMSR for IA32_FS_BASE {
    const ECX: u32 = 0xC0000100;
}

pub struct IA32_GS_BASE;
impl GenericMSR for IA32_GS_BASE {
    const ECX: u32 = 0xC0000101;
}
