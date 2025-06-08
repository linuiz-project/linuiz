#![no_std]
#![allow(non_camel_case_types)]

//! ## Safety
//!
//! It is *possible* that the current CPU doesn't support the MSR feature.
//! In this case, well... all of this fails. And we're going to ignore that.

use bit_field::BitField;

/// ## Safety
///
/// * Caller must ensure the address is valid.
#[inline]
pub unsafe fn rdmsr(address: u32) -> u64 {
    let value: u64;

    core::arch::asm!(
        "
        rdmsr
        shl rdx, 32     # Shift high value to high bits.
        or rdx, rax     # Copy low value in.
        ",
        in("ecx") address,
        out("rdx") value,
        out("rax") _,
        options(nostack, nomem)
    );

    value
}

/// ## Safety
///
/// * Caller must ensure the address is valid.
/// * Caller must ensure writing the value to the MSR address will not result in undefined behaviour.
#[inline]
pub unsafe fn wrmsr(address: u32, value: u64) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") address,
        in("rax") value,
        in("rdx") value >> 32,
        options(nostack, nomem, preserves_flags)
    );
}

macro_rules! generic_msr {
    ($name:ident, $addr:expr) => {
        pub struct $name;

        impl $name {
            #[inline]
            pub fn read() -> u64 {
                unsafe { $crate::rdmsr($addr) }
            }

            /// ## Safety
            ///
            /// Writing arbitrary data to an MSR is undefined behaviour. Caller must ensure
            /// what is written is valid for the given MSR address.
            #[inline]
            pub unsafe fn write(value: u64) {
                $crate::wrmsr($addr, value);
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
    #[inline]
    pub fn get_is_bsp() -> bool {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0x1B).get_bit(8) }
    }

    /// Gets the 10th bit of the IA32_APIC_BASE MSR, indicating the enabled state of x2 APIC mode.
    pub fn get_is_x2_mode() -> bool {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0x1B).get_bit(10) }
    }

    /// Gets the 11th bit of the IA32_APIC_BASE MSR, getting the enable state of the APIC.
    #[inline]
    pub fn get_hw_enabled() -> bool {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0x1B).get_bit(11) }
    }

    /// Gets bits 12..36 of the IA32_APIC_BASE MSR, representing the base address of the APIC.
    #[inline]
    pub fn get_base_address() -> u64 {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0x1B) & 0xFFFFFF000 }
    }
}

pub struct IA32_EFER;
impl IA32_EFER {
    // Leave the IA32_EFER.SCE bit unsupported, as we don't use `syscall`.

    /// Gets the IA32_EFER.LMA (long-mode active) bit.
    #[inline]
    pub fn get_lma() -> bool {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0xC0000080).get_bit(10) }
    }

    /// Sets the IA32_EFER.LME (long-mode enable) bit.
    ///
    /// ## Safety
    ///
    /// This function does not check if long mode is supported, or if the core is prepared to enter it.
    #[inline]
    pub unsafe fn set_lme(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(8, set));
    }

    /// Sets the IA32_EFER.SCE (syscall/syret enable) bit.
    ///
    /// ## Safety
    ///
    /// Caller must ensure software expects system calls to be enabled or disabled.
    #[inline]
    pub unsafe fn set_sce(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(0, set));
    }

    /// Gets the IA32_EFER.NXE (no-execute enable) bit.
    #[inline]
    pub fn get_nxe() -> bool {
        // Safety: MSR address is valid.
        unsafe { rdmsr(0xC0000080).get_bit(11) }
    }

    /// Sets the IA32_EFER.NXE (no-execute enable) bit.
    ///
    /// ## Safety
    ///
    /// This function does not check if the NX bit is actually supported.
    #[inline]
    pub unsafe fn set_nxe(set: bool) {
        wrmsr(0xC0000080, *rdmsr(0xC0000080).set_bit(11, set));
    }
}

pub struct IA32_STAR;
impl IA32_STAR {
    /// Sets the selectors used for `sysret`.
    ///
    /// ## Usage (from the IA32 specification):
    ///
    /// > When SYSRET transfers control to 64-bit mode user code using REX.W, the processor gets the privilege level 3
    /// > target code segment, instruction pointer, stack segment, and flags as follows:
    /// > Target code segment:       Reads a non-NULL selector from IA32_STAR\[63:48\] + 16.
    /// > ...
    /// > Target stack segment:      Reads a non-NULL selector from IA32_STAR\[63:48\] + 8
    /// > ...
    ///
    /// ## Safety
    ///
    /// Invalid low and high selectors will likely result in a #GP upon syscall.
    #[inline]
    pub unsafe fn set_selectors(kcode: u16, kdata: u16) {
        wrmsr(0xC0000081, ((kdata as u64) << 48) | ((kcode as u64) << 32));
    }
}

pub struct IA32_LSTAR;
impl IA32_LSTAR {
    /// Sets the `rip` value that's jumped to when the `syscall` instruction is executed.
    ///
    /// ## Safety
    ///
    /// Caller must ensure the given function pointer is valid for a syscall instruction pointer.
    #[inline]
    pub unsafe fn set_syscall(func: unsafe extern "sysv64" fn()) {
        wrmsr(0xC0000082, func as usize as u64);
    }
}

generic_msr!(IA32_CSTAR, 0xC0000083);

pub struct IA32_FMASK;
impl IA32_FMASK {
    /// Sets `rflags` upon a `syscall` based on masking the bits in the given value.
    ///
    /// ## Safety
    ///
    /// An invalid rflags value will result in undefined behaviour when entering a syscall handler.
    #[inline]
    pub unsafe fn set_rflags_mask(rflags: u64) {
        wrmsr(0xC0000084, rflags);
    }
}

pub struct IA32_TSC_DEADLINE;
impl IA32_TSC_DEADLINE {
    /// Sets the timestamp counter deadline.
    ///
    /// ## Safety
    ///
    /// Writing an invalid or unexpected deadline to this function could result in a deadlock.
    #[inline]
    pub unsafe fn set(value: u64) {
        wrmsr(0x6E0, value);
    }
}
