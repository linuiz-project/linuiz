#![allow(non_camel_case_types)]

//! # Safety
//!
//! It is *possible* that the current CPU doesn't support the MSR feature.
//! In this case, well... all of this fails. And we're going to ignore that.

use core::{num::NonZero, ptr::NonNull};

use crate::{
    arch::x86_64::{registers::ProcessorFlags, structures::gdt::SegmentSelector},
    cpu::{local_flags::LocalFlags, local_state::LocalState},
};
use bit_field::BitField;
use libsys::address::{Address, Virtual};

/// Implements `rdmsr` and `wrmsr` for an x86 model-specific register.
///
/// # Safety
///
/// - [`ModelSpecificRegister::REGISTER_ADDRESS`] must be the correct register
///   address for the type you're implementing.
unsafe trait ModelSpecificRegister {
    const REGISTER_ADDRESS: u32;

    /// Executes `rdmsr`, using [`ModelSpecificRegister::REGISTER_ADDRESS`] as
    /// the model-specific register address to read from.
    ///
    /// # Safety
    ///
    /// - [`ModelSpecificRegister::REGISTER_ADDRESS`] must be a supported
    ///   model-specific register on the current processor.
    #[inline(always)]
    unsafe fn rdmsr() -> u64 {
        let value_low: u64;
        let value_high: u64;

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            core::arch::asm!(
                "rdmsr",
                in("ecx") Self::REGISTER_ADDRESS,
                out("eax") value_low,
                out("edx") value_high,
                options(nostack, nomem, preserves_flags)
            );
        }

        (value_high << 32) | value_low
    }

    /// Executes `rdmsr`, using [`ModelSpecificRegister::REGISTER_ADDRESS`] as
    /// the model-specific register address to read from.
    ///
    /// # Safety
    ///
    /// - [`ModelSpecificRegister::REGISTER_ADDRESS`] must be a supported
    ///   model-specific register on the current processor.
    /// - Writing `value` to this model-specific register must not cause
    ///   undefined behaviour.
    #[inline(always)]
    unsafe fn wrmsr(value: u64) {
        let value_low = value & 0xFFFF_FFFF;
        let value_high = value >> 32;

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            core::arch::asm!(
                "wrmsr",
                in("ecx") Self::REGISTER_ADDRESS,
                in("eax") value_low,
                in("edx") value_high,
                options(nostack, nomem, preserves_flags)
            );
        }
    }
}

pub struct IA32_APIC_BASE;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_APIC_BASE {
    const REGISTER_ADDRESS: u32 = 0x1B;
}

impl IA32_APIC_BASE {
    pub fn read() -> u64 {
        // Safety: `IA32_APIC_BASE` is always supported.
        unsafe { Self::rdmsr() }
    }
}

pub struct IA32_TSC_DEADLINE;

unsafe impl ModelSpecificRegister for IA32_TSC_DEADLINE {
    const REGISTER_ADDRESS: u32 = 0x6E0;
}

pub struct IA32_EFER;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_EFER {
    const REGISTER_ADDRESS: u32 = 0xC000_0080;
}

impl IA32_EFER {
    /// Gets the `IA32_EFER.LMA` (long-mode active) bit.
    pub fn get_long_mode_active() -> bool {
        // Safety: `IA32_EFER` is always supported in long mode.
        (unsafe { Self::rdmsr() }).get_bit(10)
    }

    /// Sets the `IA32_EFER.SCE` (`syscall`/`syret` enable) bit.
    ///
    /// # Safety
    ///
    /// - Modifying this bit must not cause any undefined behaviour.
    pub unsafe fn set_sycall_enable(enable: bool) {
        // Safety: `IA32_EFER` is always supported in long mode.
        let mut bits = unsafe { Self::rdmsr() };
        let bits = *bits.set_bit(0, enable);

        // Safety:
        // - `IA32_EFER` is always supported in long mode.
        // - Caller is required to maintain all other safety invariants.
        unsafe {
            Self::wrmsr(bits);
        }
    }

    /// Gets the `IA32_EFER.NXE` (no-execute enable) bit.
    pub fn get_no_execute_enable() -> bool {
        // Safety: `IA32_EFER` is always supported in long mode.
        (unsafe { Self::rdmsr() }).get_bit(11)
    }

    /// Sets the `IA32_EFER.NXE` (no-execute enable) bit.
    ///
    /// # Safety
    ///
    /// - `NX` bit must be supported.
    ///
    /// # Remarks
    ///
    /// - Enables page access restriction by preventing instruction fetches from
    ///   PAE pages with the XD bit set.
    /// - This function does not check if the no-execute bit is supported.
    pub unsafe fn set_no_execute_enable(enable: bool) {
        // Safety: `IA32_EFER` is always supported in long mode.
        let mut bits = unsafe { Self::rdmsr() };
        let bits = *bits.set_bit(11, enable);

        // Safety:
        // - `IA32_EFER` is always supported in long mode.
        // - Caller is required to maintain all other safety invariants.
        unsafe {
            Self::wrmsr(bits);
        }
    }
}

pub struct IA32_STAR;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_STAR {
    const REGISTER_ADDRESS: u32 = 0xC000_0081;
}

impl IA32_STAR {
    /// Sets the selectors used for `sysret`.
    ///
    /// # Safety
    ///
    /// -
    ///
    /// # Remarks (from the IA32 specification):
    ///
    /// > When SYSRET transfers control to 64-bit mode user code using REX.W,
    /// > the processor gets the privilege level 3 target code segment,
    /// > instruction pointer, stack segment, and flags as follows:
    /// >
    /// > - **Target code segment**: Reads a non-NULL selector from
    /// > IA32_STAR\[63:48\] + 16.
    /// >
    /// > - **Target stack segment**: Reads a non-NULL selector from
    /// > IA32_STAR\[63:48\] + 8
    pub unsafe fn set_selectors(kcode: SegmentSelector, kdata: SegmentSelector) {
        let kcode = u64::from(kcode.as_u16());
        let kdata = u64::from(kdata.as_u16());

        let bits = (kdata << 48) | (kcode << 32);

        // Safety:
        // - `IA32_STAR` is always supported in long mode.
        // - Caller is required to maintain all other safety invariants.
        unsafe {
            Self::wrmsr(bits);
        }
    }
}

pub struct IA32_LSTAR;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_LSTAR {
    const REGISTER_ADDRESS: u32 = 0xC000_0082;
}

impl IA32_LSTAR {
    /// Sets function that's jumped to when the `syscall` instruction is
    /// executed.
    pub fn set_syscall_handler_address(address: Address<Virtual>) {
        #[allow(clippy::as_conversions)]
        Self::wrmsr(u64::try_from(func as usize).unwrap());
    }
}

pub struct IA32_CSTAR;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_CSTAR {
    const REGISTER_ADDRESS: u32 = 0xC000_0083;
}

pub struct IA32_FMASK;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_FMASK {
    const REGISTER_ADDRESS: u32 = 0xC000_0084;
}

impl IA32_FMASK {
    /// Sets `rflags` upon a `syscall` based on masking the bits in the given
    /// value.
    pub unsafe fn set(flags: ProcessorFlags) {
        let flags = u64::try_from(flags.bits()).unwrap();

        Self::wrmsr(flags);
    }
}

/// Contains the address to the [`LocalState`][crate::cpu::state::LocalState].
pub struct IA32_KERNEL_GS_BASE;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_KERNEL_GS_BASE {
    const REGISTER_ADDRESS: u32 = 0xC000_0102;
}

impl IA32_KERNEL_GS_BASE {
    pub fn get_local_state_ptr() -> Option<NonNull<LocalState>> {
        let address_bits = Self::rdmsr() & !LocalFlags::all().bits();
        let address_bits = usize::try_from(address_bits).unwrap();

        NonZero::new(address_bits).map(NonNull::with_exposed_provenance)
    }

    pub unsafe fn set_local_state_ptr(ptr: NonNull<LocalState>) {
        let local_state_address = u64::try_from(ptr.addr().get()).unwrap();
        Self::wrmsr((Self::rdmsr() & LocalFlags::all().bits()) | local_state_address);
    }

    pub fn get_local_flags() -> LocalFlags {
        LocalFlags::from_bits_truncate(Self::rdmsr())
    }

    pub unsafe fn set_local_flags(flags: LocalFlags) {
        Self::wrmsr(Self::rdmsr() | flags.bits());
    }
}

impl IA32_TSC_DEADLINE {
    /// Sets the timestamp counter deadline for the local APIC timer (if it's in
    /// TSC deadline mode).
    pub fn set(value: u64) {
        Self::wrmsr(value);
    }
}

pub struct IA32_TSC_AUX;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_TSC_AUX {
    const REGISTER_ADDRESS: u32 = 0xC000_0103;
}

impl IA32_TSC_AUX {
    /// Sets the processor ID returned by the `rdtscp` and `rdpid` instructions.
    ///
    /// # Safety
    ///
    /// - This model-specific register must be supported.
    /// - `processor_id` must be unique across the entire operating system
    ///   topography.
    pub unsafe fn set(processor_id: u32) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            Self::wrmsr(u64::from(processor_id));
        }
    }
}
