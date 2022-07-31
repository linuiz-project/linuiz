#![allow(non_camel_case_types)]

//! UNSAFETY:   It is *possible* that the current CPU doesn't support the MSR
//!             feature. In this case, well... all of this fails. And we're
//!             going to ignore that. :)
//!
//! TODO:       Don't just fail; respect the MSR feature.

use crate::{Address, Physical};
use bit_field::BitField;
use x86_64::registers::segmentation::SegmentSelector;

#[derive(Debug)]
pub struct NotSupported;

#[inline(always)]
pub unsafe fn rdmsr(ecx: u32) -> u64 {
    // TODO check the CPUID MSR feature bit

    let value: u64;
    core::arch:: asm!(
        "
        push rax        #  Preserve the `rax` value.
        rdmsr
        shl rdx, 32     # Shift high value to high bits.
        or rdx, rax     # Copy low value in.
        pop rax         # Return the preserved `rax` value.
        ",
        in("ecx") ecx,
        out("rdx") value,
        options(nostack, nomem)
    );
    value
}

#[inline(always)]
pub unsafe fn wrmsr(ecx: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") ecx,
        in("rax") value,
        in("rdx") value >> 32,
        options(nostack, nomem)
    );
}

pub trait Generic {
    const ECX: u32;

    #[inline(always)]
    fn read() -> u64 {
        unsafe { rdmsr(Self::ECX) }
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
    pub fn get_is_bsp() -> bool {
        unsafe { rdmsr(0x1B).get_bit(8) }
    }

    /// Gets the 10th bit of the IA32_APIC_BASE MSR, indicating the enabled state of x2 APIC mode.
    pub fn get_is_x2_mode() -> bool {
        unsafe { rdmsr(0x1B).get_bit(10) }
    }

    /// Gets the 11th bit of the IA32_APIC_BASE MSR, getting the enable state of the APIC.
    #[inline(always)]
    pub fn get_hw_enabled() -> bool {
        unsafe { rdmsr(0x1B).get_bit(11) }
    }

    /// Gets bits 12..36 of the IA32_APIC_BASE MSR, representing the base address of the APIC.
    #[inline]
    pub fn get_base_addr() -> Address<Physical> {
        Address::<Physical>::new((unsafe { rdmsr(0x1B) } & 0xFFFFFF000) as usize)
    }

    pub fn set(hw_enable: bool, x2_mode: bool) {
        let mut new_value = unsafe { rdmsr(0x1B) };
        new_value.set_bit(10, x2_mode);
        new_value.set_bit(11, hw_enable);
        unsafe { wrmsr(0x1B, new_value) };
    }

    #[inline]
    pub fn set_hw_enable(enable: bool) {
        unsafe { wrmsr(0x1B, *rdmsr(0x1B).set_bit(11, enable)) };
    }

    #[inline]
    pub fn set_x2_mode(enable: bool) {
        unsafe { wrmsr(0x1B, *rdmsr(0x1B).set_bit(10, enable)) };
    }
}

pub struct IA32_EFER;
impl IA32_EFER {
    /// Gets the IA32_EFER.SCE (syscall/syret enable) bit.
    #[inline(always)]
    pub fn get_sce() -> bool {
        unsafe { rdmsr(0xC0000080).get_bit(0) }
    }

    /// Sets the IA32_EFER.SCE (syscall/syret enable) bit.
    #[inline(always)]
    pub unsafe fn set_sce(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(0, set));
    }

    /// Gets the IA32_EFER.LMA (long-mode active) bit.
    #[inline(always)]
    pub fn get_lma() -> bool {
        unsafe { rdmsr(0xC0000080).get_bit(10) }
    }

    /// Sets the IA32_EFER.LME (long-mode enable) bit.
    #[inline(always)]
    pub unsafe fn set_lme(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(8, set));
    }

    /// Gets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline(always)]
    pub fn get_nxe() -> bool {
        unsafe { rdmsr(0xC0000080).get_bit(11) }
    }

    /// Sets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline(always)]
    pub unsafe fn set_nxe(set: bool) {
        assert!(
            crate::cpu::has_feature(crate::cpu::Feature::NXE),
            "Cannot enable IA32_EFER.NXE if CPU does not support it (CPUID.80000001H:EDX.NX [bit 20])."
        );

        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(11, set));
    }
}

pub struct IA32_STAR;
impl IA32_STAR {
    /// Sets the selectors used for `sysret`.
    ///
    /// Usage (from the IA32 specification):
    /// > When SYSRET transfers control to 64-bit mode user code using REX.W, the processor gets the privilege level 3
    /// > target code segment, instruction pointer, stack segment, and flags as follows:
    /// > Target code segment:       Reads a non-NULL selector from IA32_STAR[63:48] + 16.
    /// > ...
    /// > Target stack segment:      IA32_STAR[63:48] + 8
    /// > ...
    ///
    /// SAFETY: This function is unsafe because the caller must ensure the low and high selectors are valid.
    #[inline(always)]
    pub unsafe fn set_selectors(low_selector: SegmentSelector, high_selector: SegmentSelector) {
        wrmsr(0xC0000081, (high_selector.index() as u64) << 51 | (low_selector.index() as u64) << 35);
    }
}

pub struct IA32_LSTAR;
impl IA32_LSTAR {
    /// Sets the `rip` value that's jumped to when the `syscall` instruction is executed.
    ///
    /// SAFETY: This function is unsafe because the caller must ensure the given function pointer
    ///         is valid for a syscall instruction point.
    #[inline(always)]
    pub unsafe fn set_syscall(func: unsafe extern "C" fn()) {
        wrmsr(0xC0000082, func as u64);
    }
}

pub struct IA32_CSTAR;
impl Generic for IA32_CSTAR {
    const ECX: u32 = 0xC0000083;
}

pub struct IA32_SFMASK;
impl IA32_SFMASK {
    /// Sets `rflags` upon a `syscall` based on masking the bits in the given value.
    ///
    /// SAFETY: This function is unsafe because the caller must ensure the function jumped to upon
    ///         a `syscall` can correctly handle the provided RFlags.
    #[inline(always)]
    pub unsafe fn set_rflags_mask(rflags: super::RFlags) {
        wrmsr(0xC0000084, rflags.bits());
    }
}

pub struct IA32_FS_BASE;
impl Generic for IA32_FS_BASE {
    const ECX: u32 = 0xC0000100;
}

pub struct IA32_GS_BASE;
impl Generic for IA32_GS_BASE {
    const ECX: u32 = 0xC0000101;
}

pub struct IA32_KERNEL_GS_BASE;
impl Generic for IA32_KERNEL_GS_BASE {
    const ECX: u32 = 0xC0000102;
}

pub struct IA32_TSC_DEADLINE;
impl IA32_TSC_DEADLINE {
    /// Sets the timestamp counter deadline.
    ///
    /// SAFETY: Caller must ensure setting the deadline will not adversely
    ///         affect software execution.
    #[inline(always)]
    pub unsafe fn set(value: u64) {
        wrmsr(0x6E0, value);
    }
}
