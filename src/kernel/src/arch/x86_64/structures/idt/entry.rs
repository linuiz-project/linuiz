use crate::arch::x86_64::structures::{
    gdt::{KCODE_SELECTOR, PrivilegeLevel, SegmentSelector},
    idt::{InterruptStackFrame, StackTableIndex},
};
use bit_field::BitField;
use core::marker::PhantomData;
use libsys::{Address, Virtual};

/// A common trait for all handler functions usable in [`Entry`].
///
/// ## Safety
///
/// Implementors have to ensure that `get_address` returns a function address with
/// the correct signature.
pub unsafe trait HandlerFuncType {
    /// Get the virtual address of the handler function.
    fn get_address(self) -> Address<Virtual>;
}

macro_rules! impl_handler_func_type {
    ($f:ty) => {
        // Safety: `get_address` returns a function address with the correct signature.
        unsafe impl HandlerFuncType for $f {
            fn get_address(self) -> Address<Virtual> {
                // Casting a function pointer to u64 is fine, if the pointer
                // width doesn't exeed 64 bits.
                #[cfg_attr(
                    any(target_pointer_width = "32", target_pointer_width = "64"),
                    allow(clippy::fn_to_numeric_cast)
                )]
                Address::new(self as usize).unwrap()
            }
        }
    };
}

/// A handler function for an interrupt or an exception without error code.
///
/// This type alias is only usable with the `abi_x86_interrupt` feature enabled.
pub type HandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame);
impl_handler_func_type!(HandlerFunc);

/// A handler function for an exception that pushes an error code.
///
/// This type alias is only usable with the `abi_x86_interrupt` feature enabled.
pub type HandlerFuncWithErrCode = extern "x86-interrupt" fn(InterruptStackFrame, error_code: u64);
impl_handler_func_type!(HandlerFuncWithErrCode);

/// A page fault handler function that pushes a page fault error code.
///
/// This type alias is only usable with the `abi_x86_interrupt` feature enabled.
pub type PageFaultHandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame, error_code: super::PageFaultErrorCode);
impl_handler_func_type!(PageFaultHandlerFunc);

/// A handler function that must not return, e.g. for a machine check exception.
///
/// This type alias is only usable with the `abi_x86_interrupt` feature enabled.
pub type DivergingHandlerFunc = extern "x86-interrupt" fn(InterruptStackFrame) -> !;
impl_handler_func_type!(DivergingHandlerFunc);

/// A handler function with an error code that must not return, e.g. for a double fault exception.
///
/// This type alias is only usable with the `abi_x86_interrupt` feature enabled.
pub type DivergingHandlerFuncWithErrCode = extern "x86-interrupt" fn(InterruptStackFrame, error_code: u64) -> !;
impl_handler_func_type!(DivergingHandlerFuncWithErrCode);

/// A general handler function for an interrupt or an exception with the interrupt/exceptions's index and an optional error code.
pub type GeneralHandlerFunc = fn(InterruptStackFrame, index: u8, error_code: Option<u64>);
impl_handler_func_type!(GeneralHandlerFunc);

/// An Interrupt Descriptor Table entry.
///
/// The generic parameter is some [`HandlerFuncType`], depending on the interrupt vector.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry<F> {
    pointer_low: u16,
    options: Options,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
    phantom: PhantomData<F>,
}

impl<F> core::fmt::Debug for Entry<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Entry")
            .field("handler_addr", &self.handler_addr())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl<F> PartialEq for Entry<F> {
    fn eq(&self, other: &Self) -> bool {
        self.pointer_low == other.pointer_low
            && self.options == other.options
            && self.pointer_middle == other.pointer_middle
            && self.pointer_high == other.pointer_high
            && self.reserved == other.reserved
    }
}

impl<F> Entry<F> {
    /// Creates a non-present IDT entry (but sets the must-be-one bits).
    #[inline]
    pub const fn missing() -> Self {
        Entry {
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: Options::minimal(),
            reserved: 0,
            phantom: PhantomData,
        }
    }

    /// Configure the interrupt descriptor table entry.
    ///
    /// ## Safety
    /// - `address` must point to a valid address for the
    unsafe fn set_handler_addr(&mut self, fn_address: Address<Virtual>) -> &mut Options {
        let fn_address = fn_address.get();

        self.pointer_low = u16::try_from(fn_address.get_bits(..16)).unwrap();
        self.pointer_middle = u16::try_from(fn_address.get_bits(16..32)).unwrap();
        self.pointer_high = u32::try_from(fn_address.get_bits(32..64)).unwrap();

        self.options = Options::minimal();
        // Safety: `KCODE_SELECTOR` is a valid segment selector for the kernel code segment.
        unsafe {
            self.options.set_code_selector(KCODE_SELECTOR);
        }
        self.options.set_present(true);

        &mut self.options
    }

    pub fn handler_addr(&self) -> Address<Virtual> {
        Address::new_truncate(
            ((self.pointer_high as usize) << 32) | ((self.pointer_middle as usize) << 16) | (self.pointer_low as usize),
        )
    }
}

impl<F: HandlerFuncType> Entry<F> {
    /// Sets the handler function for the IDT entry and sets the following defaults:
    ///   - The code selector is the code segment currently active in the CPU
    ///   - The present bit is set
    ///   - Interrupts are disabled on handler invocation
    ///   - The privilege level (DPL) is [`PrivilegeLevel::Ring0`]
    ///   - No IST is configured (existing stack will be used)
    ///
    /// The function returns a mutable reference to the entry's options that allows
    /// further customization.
    ///
    /// This method is only usable with the `abi_x86_interrupt` feature enabled. Without it, the
    /// unsafe [`Entry::set_handler_addr`] method has to be used instead.
    fn set_handler_fn(&mut self, handler: F) -> &mut Options {
        // Safety: Caller is required to ensure the provided function correctly handles
        //         the interrupt assocaited with this `Entry`.
        unsafe { self.set_handler_addr(handler.get_address()) }
    }

    /// Sets the handler function for the IDT entry and sets the following defaults:
    ///   - The code selector is the kernel code segment.
    ///   - The present bit is set.
    ///   - Interrupts are disabled on handler invocation.
    ///   - The privilege level (DPL) is [`PrivilegeLevel::Ring0`].
    ///   - No interrupt stack table is configured (existing stack will be used).
    ///
    /// The function returns a mutable reference to the entry's options that allows
    /// further customization.
    ///
    /// This method is only usable with the `abi_x86_interrupt` feature enabled. Without it, the
    /// unsafe [`Entry::set_handler_addr`] method has to be used instead.
    pub fn new(handler: F) -> Self {
        let mut entry = Self::missing();
        entry.set_handler_fn(handler);
        entry
    }

    /// Sets the handler function for the IDT entry and sets the following defaults:
    ///   - The code selector is the kernel code segment.
    ///   - The present bit is set.
    ///   - Interrupts are disabled on handler invocation.
    ///   - The privilege level (DPL) is [`PrivilegeLevel::Ring0`].
    ///   - The interrupt stack table index is set to `stack_table_index`.
    ///
    /// The function returns a mutable reference to the entry's options that allows
    /// further customization.
    ///
    /// This method is only usable with the `abi_x86_interrupt` feature enabled. Without it, the
    /// unsafe [`Entry::set_handler_addr`] method has to be used instead.
    ///
    /// ## Safety
    ///
    /// TODO
    pub unsafe fn new_with_stack(handler: F, stack_table_index: StackTableIndex) -> Self {
        let mut entry = Self::missing();
        let options = entry.set_handler_fn(handler);

        // Safety: Caller is required to guarantee the stack table index is correct.
        unsafe {
            options.set_stack_index(stack_table_index);
        }

        entry
    }

    /// Sets the handler function for the IDT entry and sets the following defaults:
    ///   - The code selector is the code segment currently active in the CPU
    ///   - The present bit is set.
    ///   - Interrupts are disabled on handler invocation.
    ///   - The privilege level (DPL) set to `privilege_level`.
    ///   - No interrupt stack table is configured (existing stack will be used).
    ///
    /// The function returns a mutable reference to the entry's options that allows
    /// further customization.
    ///
    /// This method is only usable with the `abi_x86_interrupt` feature enabled. Without it, the
    /// unsafe [`Entry::set_handler_addr`] method has to be used instead.
    ///
    /// ## Safety
    ///
    /// TODO
    pub unsafe fn new_with_privilege(handler: F, privilege_level: PrivilegeLevel) -> Self {
        let mut entry = Self::missing();
        let options = entry.set_handler_fn(handler);

        // Safety: Caller is required to guarantee the stack table index is correct.
        unsafe {
            options.set_privilege_level(privilege_level);
        }

        entry
    }
}

/// Represents the 4 non-offset bytes of an IDT entry.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Options {
    cs: SegmentSelector,
    bits: u16,
}

impl core::fmt::Debug for Options {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("EntryOptions")
            .field("code_selector", &self.cs)
            .field("stack_index", &(self.bits.get_bits(0..3) - 1))
            .field("type", &format_args!("{:#04b}", self.bits.get_bits(8..12)))
            .field("privilege_level", &PrivilegeLevel::from_u16(self.bits.get_bits(13..15)))
            .field("present", &self.bits.get_bit(15))
            .finish()
    }
}

impl Options {
    /// Creates a minimal options field with all the must-be-one bits set. This
    /// means the CS selector, IST, and DPL field are all 0.
    #[inline]
    const fn minimal() -> Self {
        Options {
            cs: SegmentSelector::NULL,
            bits: 0b1110_0000_0000, // Default to a 64-bit Interrupt Gate
        }
    }

    /// Set the code segment that will be used by this interrupt.
    ///
    /// ## Safety
    ///
    ///  - `cs` must select a valid, long-mode code segment.
    pub const unsafe fn set_code_selector(&mut self, cs: SegmentSelector) -> &mut Self {
        self.cs = cs;
        self
    }

    /// Set or reset the preset bit.
    ///
    /// ## Safety
    ///
    /// TODO
    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.bits.set_bit(15, present);
        self
    }

    /// Let the CPU disable hardware interrupts when the handler is invoked. By default,
    /// interrupts are disabled on handler invocation.
    ///
    /// ## Safety
    ///
    /// TODO
    pub unsafe fn set_disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.bits.set_bit(8, !disable);
        self
    }

    /// Set the required privilege level (DPL) for invoking the handler. The DPL can be 0, 1, 2,
    /// or 3, the default is 0. If CPL < DPL, a general protection fault occurs.
    ///
    /// ## Safety
    ///
    /// TODO
    pub unsafe fn set_privilege_level(&mut self, dpl: PrivilegeLevel) -> &mut Self {
        self.bits.set_bits(13..15, dpl as u16);
        self
    }

    /// Assigns a Interrupt Stack Table (IST) stack to this handler. The CPU will then always
    /// switch to the specified stack before the handler is invoked. This allows kernels to
    /// recover from corrupt stack pointers (e.g., on kernel stack overflow).
    ///
    /// An IST stack is specified by an IST index between 0 and 6 (inclusive). Using the same
    /// stack for multiple interrupts can be dangerous when nested interrupts are possible.
    ///
    /// This function panics if the index is not in the range 0..7.
    ///
    /// ## Safety
    ///
    /// This function is unsafe because the caller must ensure that the passed stack index is
    /// valid and not used by other interrupts. Otherwise, memory safety violations are possible.
    pub unsafe fn set_stack_index(&mut self, index: StackTableIndex) -> &mut Self {
        // The hardware IST index starts at 1, but our software IST index
        // starts at 0. Therefore we need to add 1 here.
        self.bits.set_bits(0..3, (index as u16) + 1);
        self
    }
}
