#![allow(non_camel_case_types)]

//! UNSAFETY:   It is *possible* that the current CPU doesn't support the MSR
//!             feature. In this case, well... all of this fails. And we're
//!             going to ignore that. :)
//!
//! TODO:       Don't just fail; respect the MSR feature.

use bit_field::BitField;
use libkernel::{Address, Physical};

#[derive(Debug)]
pub struct NotSupported;

/// SAFETY: This function does not check if MSRs are supported by this core, or if the procided MSR address is valid.
#[inline(always)]
pub unsafe fn rdmsr(ecx: u32) -> u64 {
    // TODO check the CPUID MSR feature bit

    let value: u64;
    core::arch::asm!(
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

/// SAFETY: This function does not check if MSRs are supported by this core, or if the procided MSR address is valid.
///         This function does not check whether the provided write to the provided MSR address will result in undefined behaviour.
#[inline(always)]
pub unsafe fn wrmsr(ecx: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") ecx,
        in("rax") value,
        in("rdx") value >> 32,
        options(nostack, nomem, preserves_flags)
    );
}

macro_rules! generic_msr {
    ($name:ident, $addr:expr) => {
        pub struct $name;

        impl $name {
            #[inline(always)]
            pub fn read() -> u64 {
                unsafe { $crate::arch::x64::registers::msr::rdmsr($addr) }
            }

            #[inline(always)]
            pub unsafe fn write(value: u64) {
                $crate::arch::x64::registers::msr::wrmsr($addr, value);
            }
        }
    };
}

generic_msr!(IA32_FS_BASE, 0xC0000100);
generic_msr!(IA32_GS_BASE, 0xC0000101);
generic_msr!(IA32_KERNEL_GS_BASE, 0xC0000102);

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
    pub fn get_base_address() -> Address<Physical> {
        Address::<Physical>::new_truncate(unsafe { rdmsr(0x1B) } & 0xFFFFFF000)
    }
}

pub struct IA32_EFER;
impl IA32_EFER {
    /// Leave the IA32_EFER.SCE bit unsupported, as we don't use `syscall`.

    /// Gets the IA32_EFER.LMA (long-mode active) bit.
    #[inline(always)]
    pub fn get_lma() -> bool {
        unsafe { rdmsr(0xC0000080).get_bit(10) }
    }

    /// Sets the IA32_EFER.LME (long-mode enable) bit.
    ///
    /// SAFETY: This function does not check if long mode is supported, or if the core is prepared to enter it.
    #[inline(always)]
    pub unsafe fn set_lme(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(8, set));
    }

    /// Sets the IA32_EFER.SCE (syscall/syret enable) bit.
    ///
    /// SAFETY: Caller must ensure software expects system calls to be enabled or disabled.
    #[inline(always)]
    pub unsafe fn set_sce(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(0, set));
    }

    /// Gets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline(always)]
    pub fn get_nxe() -> bool {
        unsafe { rdmsr(0xC0000080).get_bit(11) }
    }

    /// Sets the IA32_EFER.NXE (no-execute enable) bit.
    ///
    /// SAFETY: This function does not check if the NX bit is actually supported.
    ///         Undefined behaviour will result if the NX bit is used in a page table entry, and this bit is disabled.
    #[inline(always)]
    pub unsafe fn set_nxe(set: bool) {
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
    /// > Target code segment:       Reads a non-NULL selector from IA32_STAR\[63:48\] + 16.
    /// > ...
    /// > Target stack segment:      IA32_STAR\[63:48\] + 8
    /// > ...
    ///
    /// SAFETY: Caller must ensure the low and high selectors are valid.
    #[inline(always)]
    pub unsafe fn set_selectors(
        low_selector: x86_64::structures::gdt::SegmentSelector,
        high_selector: x86_64::structures::gdt::SegmentSelector,
    ) {
        wrmsr(0xC0000081, (high_selector.index() as u64) << 51 | (low_selector.index() as u64) << 35);
    }
}

pub struct IA32_LSTAR;
impl IA32_LSTAR {
    /// Sets the `rip` value that's jumped to when the `syscall` instruction is executed.
    ///
    /// SAFETY: Caller must ensure the given function pointer is valid for a syscall instruction pointer.
    #[inline(always)]
    pub unsafe fn set_syscall(func: unsafe extern "C" fn()) {
        wrmsr(0xC0000082, func as u64);
    }
}

generic_msr!(IA32_CSTAR, 0xC0000083);

pub struct IA32_SFMASK;
impl IA32_SFMASK {
    /// Sets `rflags` upon a `syscall` based on masking the bits in the given value.
    ///
    /// SAFETY: Caller must ensure the function jumped to upon a `syscall` can correctly handle the provided RFlags.
    #[inline(always)]
    pub unsafe fn set_rflags_mask(rflags: super::RFlags) {
        wrmsr(0xC0000084, rflags.bits());
    }
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
