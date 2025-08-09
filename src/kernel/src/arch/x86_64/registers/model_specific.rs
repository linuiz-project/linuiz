#![allow(non_camel_case_types)]

use crate::cpu::local_state::LocalState;
use bit_field::BitField;
use core::{num::NonZero, ptr::NonNull};

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

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_TSC_DEADLINE {
    const REGISTER_ADDRESS: u32 = 0x6E0;
}

pub struct IA32_EFER;

// Safety: `REGISTER_ADDRESS` is correct.
unsafe impl ModelSpecificRegister for IA32_EFER {
    const REGISTER_ADDRESS: u32 = 0xC000_0080;
}

impl IA32_EFER {
    fn read() -> u64 {
        // Safety: `IA32_KERNEL_GS_BASE` is always supported in long mode.
        unsafe { Self::rdmsr() }
    }

    /// Gets the `IA32_EFER.LMA` (long-mode active) bit.
    pub fn get_long_mode_active() -> bool {
        Self::read().get_bit(10)
    }

    /// Gets the `IA32_EFER.NXE` (no-execute enable) bit.
    pub fn get_no_execute_enable() -> bool {
        Self::read().get_bit(11)
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
        let bits = *Self::read().set_bit(11, enable);

        // Safety:
        // - `IA32_EFER` is always supported in long mode.
        // - Caller is required to maintain all other safety invariants.
        unsafe {
            Self::wrmsr(bits);
        }
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
        // Safety: `IA32_KERNEL_GS_BASE` is always supported in long mode.
        let bits = unsafe { Self::rdmsr() };

        // We would like to avoid potentionally panicking in this function.
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let address_bits = bits as usize;

        NonZero::new(address_bits).map(NonNull::with_exposed_provenance)
    }

    /// Sets the processor-local pointer to the [`LocalState`] structure.
    pub unsafe fn set_local_state_ptr(ptr: NonNull<LocalState>) {
        // We would like to avoid potentionally panicking in this function.
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let address_bits = ptr.addr().get() as u64;

        // Safety:
        // - `IA32_KERNEL_GS_BASE` is always supported in long mode.
        // - Caller is required to maintain all other safety invariants.
        unsafe {
            Self::wrmsr(address_bits);
        }
    }
}

impl IA32_TSC_DEADLINE {
    /// Sets the timestamp counter deadline for the local APIC timer (if it's in
    /// TSC deadline mode).
    ///
    /// # Safety
    ///
    /// - `IA32_TSC_DEADLINE` model-specific register must be supported.
    pub unsafe fn set(value: u64) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            Self::wrmsr(value);
        }
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
