#![allow(non_camel_case_types)]

//! UNSAFETY:   It is *possible* that the current CPU doesn't support the MSR
//!             feature. In this case, well... all of this fails. And we're
//!             going to ignore that. :)
//!
//! TODO:       Don't just fail; respect the MSR feature.

use crate::{Address, Physical};
use bit_field::BitField;

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
        options(nostack, nomem, preserves_flags)
    );
}

macro_rules! generic_msr {
    ($name:ident, $addr:expr) => {
        pub struct $name;

        impl $name {
            #[inline(always)]
            pub fn read() -> u64 {
                unsafe { $crate::registers::msr::rdmsr($addr) }
            }

            #[inline(always)]
            pub unsafe fn write(value: u64) {
                $crate::registers::msr::wrmsr($addr, value);
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
    /// Leave the IA32_EFER.SCE bit unsupported, as we don't use `syscall`.

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
            crate::cpu::EXT_FUNCTION_INFO
                .as_ref()
                .map(|ext_func_info| ext_func_info.has_execute_disable())
                .unwrap_or(false),
            "Cannot enable IA32_EFER.NXE if CPU does not support it (CPUID.80000001H:EDX.NX [bit 20])."
        );

        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(11, set));
    }
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
