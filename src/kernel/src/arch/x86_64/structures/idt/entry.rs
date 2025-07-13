use crate::arch::x86_64::structures::{
    gdt::{KCODE_SELECTOR, PrivilegeLevel, SegmentSelector},
    tss::InterruptStackTableIndex,
};
use bit_field::BitField;

/// An Interrupt Descriptor Table entry.
///
/// The generic parameter is some [`HandlerFuncType`], depending on the interrupt vector.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entry {
    pointer_low: u16,
    options: Options,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Entry")
            .field("handler_addr", &self.handler_addr())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.pointer_low == other.pointer_low
            && self.options == other.options
            && self.pointer_middle == other.pointer_middle
            && self.pointer_high == other.pointer_high
            && self.reserved == other.reserved
    }
}

impl Entry {
    /// Creates a non-present IDT entry (but sets the must-be-one bits).
    pub const fn missing() -> Self {
        Entry {
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: Options::minimal(),
            reserved: 0,
        }
    }

    /// # Safety
    ///
    /// - `address` must be a valid address that points to a function that will
    ///   handle and call into the requisite interrupt handler.
    pub unsafe fn new(address: usize) -> Self {
        let mut entry = Entry::missing();

        entry.pointer_low = u16::try_from(address.get_bits(..16)).unwrap();
        entry.pointer_middle = u16::try_from(address.get_bits(16..32)).unwrap();
        entry.pointer_high = u32::try_from(address.get_bits(32..64)).unwrap();

        // Safety: `KCODE_SELECTOR` is the correct segment selector for the kernel code segment.
        unsafe {
            entry.options.set_code_selector(*KCODE_SELECTOR.wait());
        }

        entry.options.set_present(true);

        entry
    }

    /// # Safety
    ///
    /// - `address` must be a valid address that points to a function that will
    ///   handle and call into the requisite interrupt handler.
    /// - `interrupt_stack_table_index` must be the correct stack table index
    ///   associated with the interrupt.
    pub unsafe fn new_with_stack(
        address: usize,
        interrupt_stack_table_index: InterruptStackTableIndex,
    ) -> Self {
        // Safety: Caller is required to maintain invariants.
        let mut entry = unsafe { Self::new(address) };

        // Safety: Caller is required to guarantee the stack table index is correct.
        unsafe {
            entry.options.set_stack_index(interrupt_stack_table_index);
        }

        entry
    }

    /// # Safety
    ///
    /// - `address` must be a valid address that points to a function that will
    //    handle and call into the requisite interrupt handler.
    /// - `privilege_level` must be the correct privilege level that software is
    ///   required to jump to upon interrupt entry.
    pub unsafe fn new_with_privilege(address: usize, privilege_level: PrivilegeLevel) -> Self {
        // Safety: Caller is required to maintain invariants.
        let mut entry = unsafe { Self::new(address) };

        // Safety: Caller is required to guarantee the stack table index is correct.
        unsafe {
            entry.options.set_privilege_level(privilege_level);
        }

        entry
    }

    fn handler_addr(&self) -> u64 {
        (u64::from(self.pointer_high) << 32)
            | (u64::from(self.pointer_middle) << 16)
            | u64::from(self.pointer_low)
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
            .field(
                "privilege_level",
                &PrivilegeLevel::try_from(self.bits.get_bits(13..15)).unwrap(),
            )
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
    pub unsafe fn set_privilege_level(&mut self, value: PrivilegeLevel) -> &mut Self {
        self.bits.set_bits(13..15, u16::from(value));
        self
    }

    /// Assigns an interrupt stack table (IST) stack to this handler. The CPU will then always
    /// switch to the specified stack before the handler is invoked. This allows kernels to
    /// recover from corrupt stack pointers (e.g. on kernel stack overflow).
    ///
    /// An interrupt stack table stack is specified by an index between 0..=6. Using the same
    /// stack for multiple interrupts can be dangerous when nested interrupts are possible.
    ///
    /// This function panics if the index is not in the range 0..=6.
    ///
    /// # Safety
    ///
    /// This function is unsafe because the caller must ensure that the passed stack index is
    /// valid and not used by other interrupts. Otherwise, memory safety violations are possible.
    pub unsafe fn set_stack_index(&mut self, index: InterruptStackTableIndex) -> &mut Self {
        // The hardware IST index starts at 1, but our software IST index
        // starts at 0. Therefore we need to add 1 here.
        self.bits.set_bits(0..3, u16::from(index) + 1);
        self
    }
}
