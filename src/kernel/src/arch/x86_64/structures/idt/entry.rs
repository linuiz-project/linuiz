use crate::arch::x86_64::structures::{
    gdt::{KCODE_SELECTOR, PrivilegeLevel, SegmentSelector},
    tss::InterruptStackTableIndex,
};
use bit_field::BitField;

pub struct EntryBuilder {
    handler_address: usize,
    code_selector: SegmentSelector,
    options: u16,
}

impl EntryBuilder {
    pub fn exception() -> Self {
        Self {
            handler_address: 0,
            code_selector: *KCODE_SELECTOR.get().unwrap(),
            options: 0b0000_1111_0000_0000,
        }
    }

    pub fn interrupt_service_routine() -> Self {
        Self {
            handler_address: 0,
            code_selector: *KCODE_SELECTOR.get().unwrap(),
            options: 0b0000_1110_0000_0000,
        }
    }

    /// # Safety
    ///
    /// - `address` must be a valid address that points to a function that will
    ///   correctly handle this interrupt vector.
    pub unsafe fn with_handler(mut self, address: usize) -> Self {
        self.handler_address = address;

        self
    }

    /// Assigns an interrupt stack table (IST) stack to this handler. The CPU
    /// will then always switch to the specified stack before the handler is
    /// invoked. This allows kernels to recover from corrupted stack pointers
    /// (e.g. on kernel stack overflow).
    ///
    /// # Remarks
    ///
    /// Using the same stack for multiple interrupts can be dangerous if nested
    /// interrupts are enabled.
    ///
    /// # Safety
    ///
    /// - `interrupt_stack_table_index` must be the correct stack table index
    ///   associated with the interrupt.
    pub unsafe fn with_interrupt_stack_table_index(
        mut self,
        interrupt_stack_table_index: InterruptStackTableIndex,
    ) -> Self {
        self.options
            .set_bits(0..3, u16::from(interrupt_stack_table_index));

        self
    }

    /// # Safety
    ///
    /// - `privilege_level` must be the correct privilege level that software is
    ///   required to jump to upon interrupt entry.
    pub unsafe fn with_privilege_level(mut self, privilege_level: PrivilegeLevel) -> Self {
        self.options.set_bits(13..15, u16::from(privilege_level));

        self
    }

    pub fn build(mut self) -> Entry {
        // Set the `present` bit.
        self.options.set_bit(15, true);

        let mut entry_bytes = [0u8; 16];

        let pointer_bytes = self.handler_address.to_le_bytes();
        let code_selector_bytes = self.code_selector.as_u16().to_le_bytes();
        let options_bytes = self.options.to_le_bytes();

        entry_bytes[0..2].copy_from_slice(&pointer_bytes[0..2]); // Address low bytes.
        entry_bytes[6..8].copy_from_slice(&pointer_bytes[2..4]); // Address middle bytes.
        entry_bytes[8..12].copy_from_slice(&pointer_bytes[4..8]); // Address high bytes.
        entry_bytes[2..4].copy_from_slice(&code_selector_bytes); // Code selector.
        entry_bytes[4..6].copy_from_slice(&options_bytes); // Options.

        Entry(entry_bytes)
    }
}

/// An [`InterruptDescriptorTable`][crate::arch::x86_64::structures::idt::InterruptDescriptorTable] entry.
#[repr(transparent)]
#[derive(Clone)]
pub struct Entry([u8; 16]);

impl Entry {
    /// Creates a non-present IDT entry (but sets the must-be-one bits).
    pub const fn missing() -> Self {
        Self([0u8; _])
    }

    pub fn handler_address(&self) -> u64 {
        let low_bits = u64::from(u16::from_le_bytes(self.0[0..2].try_into().unwrap()));
        let mid_bits = u64::from(u16::from_le_bytes(self.0[6..8].try_into().unwrap()));
        let high_bits = u64::from(u32::from_le_bytes(self.0[8..12].try_into().unwrap()));

        (high_bits << 32) | (mid_bits << 16) | low_bits
    }

    pub fn code_selector(&self) -> SegmentSelector {
        let bits = u16::from_le_bytes(self.0[2..4].try_into().unwrap());

        SegmentSelector::from(bits)
    }

    pub fn options(&self) -> u16 {
        u16::from_le_bytes(self.0[4..6].try_into().unwrap())
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Entry")
            .field("Handler Address", &self.handler_address())
            .field("Code Selector", &self.code_selector())
            .field("Options", &self.options())
            .finish()
    }
}
