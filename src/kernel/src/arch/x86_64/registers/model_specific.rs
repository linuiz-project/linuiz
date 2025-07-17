#![allow(non_camel_case_types)]

//! # Safety
//!
//! It is *possible* that the current CPU doesn't support the MSR feature.
//! In this case, well... all of this fails. And we're going to ignore that.

use core::{num::NonZero, ptr::NonNull};

use crate::{
    arch::x86_64::{registers::RFlags, structures::gdt::SegmentSelector},
    cpu::local_state::LocalState,
};
use bit_field::BitField;
use libsys::{Address, Virtual};

/// # Safety
///
/// - `address` must be a valid MSR.
/// - Caller mu
#[inline(always)]
fn rdmsr<T: ModelSpecificRegister>() -> u64 {
    let value_low: u64;
    let value_high: u64;

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") T::REGISTER_ADDRESS,
            out("eax") value_low,
            out("edx") value_high,
            options(nostack, nomem, preserves_flags)
        );
    }

    (value_high << 32) | value_low
}

/// ## Safety
///
/// * Caller must ensure the address is valid.
/// * Caller must ensure writing the value to the MSR address will not result in undefined behaviour.
#[inline(always)]
fn wrmsr<T: ModelSpecificRegister>(value: u64) {
    let value_low = value & 0xFFFF_FFFF;
    let value_high = value >> 32;

    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") T::REGISTER_ADDRESS,
            in("eax") value_low,
            in("edx") value_high,
            options(nostack, nomem, preserves_flags)
        );
    }
}

trait ModelSpecificRegister {
    const REGISTER_ADDRESS: u32;
}

/// Contains the address to the [`LocalState`][crate::cpu::state::LocalState].
pub struct IA32_KERNEL_GS_BASE;

impl ModelSpecificRegister for IA32_KERNEL_GS_BASE {
    const REGISTER_ADDRESS: u32 = 0xC0000102;
}

impl IA32_KERNEL_GS_BASE {
    pub fn write(ptr: NonNull<LocalState>) {
        wrmsr::<Self>(NonZero::<u64>::try_from(ptr.addr()).unwrap().get());
    }

    pub fn read() -> Option<NonNull<LocalState>> {
        usize::try_from(rdmsr::<Self>())
            .ok()
            .and_then(NonZero::new)
            .map(NonNull::with_exposed_provenance)
    }
}

pub struct IA32_APIC_BASE;

impl ModelSpecificRegister for IA32_APIC_BASE {
    const REGISTER_ADDRESS: u32 = 0x1B;
}

impl IA32_APIC_BASE {
    /// Indicates whether the current hardware thread in the bootstrap hardware thread ('bootstrap processor').
    pub fn get_is_bsp() -> bool {
        rdmsr::<Self>().get_bit(8)
    }

    /// Indicates whether the local APIC is operating in x2 mode.
    pub fn get_is_x2apic_mode() -> bool {
        rdmsr::<Self>().get_bit(10)
    }

    /// Gets the enable state of the APIC.
    pub fn get_hw_enabled() -> bool {
        rdmsr::<Self>().get_bit(11)
    }

    /// Sets the enable state of the APIC.
    pub fn set_hw_enabled(enable: bool) {
        wrmsr::<Self>(*rdmsr::<Self>().set_bit(11, enable));
    }

    /// Gets the base address of the local APIC.
    pub fn get_base_address() -> Address<Virtual> {
        let base_address = usize::try_from(rdmsr::<Self>())
            .expect("could not convert `IA32_APIC_BASE` to `usize`");

        Address::new(base_address).expect("`IA32_APIC_BASE` returned an invalid address")
    }
}

pub struct IA32_EFER;

impl ModelSpecificRegister for IA32_EFER {
    const REGISTER_ADDRESS: u32 = 0xC0000080;
}

impl IA32_EFER {
    /// Gets the `IA32_EFER.LMA` (long-mode active) bit.
    pub fn get_long_mode_active() -> bool {
        rdmsr::<Self>().get_bit(10)
    }

    /// Sets the `IA32_EFER.LME` (long-mode enable) bit.
    pub unsafe fn set_long_mode_enable(enable: bool) {
        wrmsr::<Self>(*rdmsr::<Self>().set_bit(8, enable));
    }

    /// Sets the `IA32_EFER.SCE` (`syscall`/`syret` enable) bit.
    pub fn set_sycall_enable(enable: bool) {
        wrmsr::<Self>(*rdmsr::<Self>().set_bit(0, enable));
    }

    /// Gets the `IA32_EFER.NXE` (no-execute enable) bit.
    pub fn get_no_execute_enable() -> bool {
        rdmsr::<Self>().get_bit(11)
    }

    /// Sets the `IA32_EFER.NXE` (no-execute enable) bit.
    ///
    /// # Remarks
    ///
    /// - Enables page access restriction by preventing instruction fetches
    ///   from PAE pages with the XD bit set.
    /// - This function does not check if the no-execute bit is supported.
    pub fn set_no_execute_enable(enable: bool) {
        wrmsr::<Self>(*rdmsr::<Self>().set_bit(11, enable));
    }
}

pub struct IA32_STAR;

impl ModelSpecificRegister for IA32_STAR {
    const REGISTER_ADDRESS: u32 = 0xC0000081;
}

impl IA32_STAR {
    /// Sets the selectors used for `sysret`.
    ///
    /// # Usage (from the IA32 specification):
    ///
    /// > When SYSRET transfers control to 64-bit mode user code using REX.W, the processor gets the privilege level 3
    /// > target code segment, instruction pointer, stack segment, and flags as follows:
    /// > Target code segment:       Reads a non-NULL selector from IA32_STAR\[63:48\] + 16.
    /// > ...
    /// > Target stack segment:      Reads a non-NULL selector from IA32_STAR\[63:48\] + 8
    /// > ...
    ///
    pub unsafe fn set_selectors(kcode: SegmentSelector, kdata: SegmentSelector) {
        let kcode = u64::from(kcode.as_u16());
        let kdata = u64::from(kdata.as_u16());

        wrmsr::<Self>((kdata << 48) | (kcode << 32));
    }
}

pub struct IA32_LSTAR;

impl ModelSpecificRegister for IA32_LSTAR {
    const REGISTER_ADDRESS: u32 = 0xC0000082;
}

impl IA32_LSTAR {
    /// Sets function that's jumped to when the `syscall` instruction is executed.
    pub fn set_syscall(func: unsafe extern "sysv64" fn()) {
        #[allow(clippy::as_conversions)]
        wrmsr::<Self>(u64::try_from(func as usize).unwrap());
    }
}

pub struct IA32_CSTAR;

impl ModelSpecificRegister for IA32_CSTAR {
    const REGISTER_ADDRESS: u32 = 0xC0000083;
}

pub struct IA32_FMASK;

impl ModelSpecificRegister for IA32_FMASK {
    const REGISTER_ADDRESS: u32 = 0xC0000084;
}

impl IA32_FMASK {
    /// Sets `rflags` upon a `syscall` based on masking the bits in the given value.
    pub unsafe fn set(rflags: RFlags) {
        wrmsr::<Self>(rflags.bits());
    }
}

pub struct IA32_TSC_DEADLINE;

impl ModelSpecificRegister for IA32_TSC_DEADLINE {
    const REGISTER_ADDRESS: u32 = 0x6E0;
}

impl IA32_TSC_DEADLINE {
    /// Sets the timestamp counter deadline for the local APIC timer (if it's in TSC deadline mode).
    pub fn set(value: u64) {
        wrmsr::<Self>(value);
    }
}
