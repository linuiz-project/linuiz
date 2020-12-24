use crate::{structures::idt::InterruptStackFrame, PrivilegeLevel};
use bit_field::BitField;
use core::marker::PhantomData;

pub type InterruptHandler = extern "x86-interrupt" fn(&mut InterruptStackFrame);
pub type InterruptHandlerWithErrCode =
    extern "x86-interrupt" fn(&mut InterruptStackFrame, error_code: u64);
pub type PageFaultHandler = extern "x86-interrupt" fn(
    &mut InterruptStackFrame,
    error_code: crate::structures::idt::PageFaultError,
);
pub type DivergingHandler = extern "x86-interrupt" fn(&mut InterruptStackFrame) -> !;
pub type DivergingHandlerWithErrCode =
    extern "x86-interrupt" fn(&mut InterruptStackFrame, error_code: u64) -> !;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InterruptVector<F> {
    pointer_low: u16,
    gdt_selector: u16,
    options: InterruptVectorOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
    phantom: PhantomData<F>,
}

impl<F> InterruptVector<F> {
    /// Creates a non-present IDT vector (but provides minimal options for a valid vector).
    pub const fn missing() -> Self {
        Self {
            gdt_selector: 0,
            pointer_low: 0,
            pointer_middle: 0,
            pointer_high: 0,
            options: InterruptVectorOptions::minimal(),
            reserved: 0,
            phantom: PhantomData,
        }
    }

    fn set_handler_addr(&mut self, addr: u64) -> &mut InterruptVectorOptions {
        self.pointer_low = addr as u16; // capture lower 16 bits
        self.pointer_middle = (addr >> 16) as u16; // capture 16..32
        self.pointer_high = (addr >> 32) as u32; // capture 32..64
        self.gdt_selector = crate::instructions::cs().0;
        self.options.set_present(true);

        &mut self.options
    }

    pub fn set_handler(&mut self, handler: F) -> &mut InterruptVectorOptions {
        self.set_handler_addr(&handler as *const F as u64)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptVectorOptions(u16);

impl InterruptVectorOptions {
    const fn minimal() -> Self {
        InterruptVectorOptions(0b1110_0000_0000)
    }

    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.0.set_bit(15, present);
        self
    }

    pub fn toggle_interrupts(&mut self, enable: bool) -> &mut Self {
        self.0.set_bit(8, enable);
        self
    }

    pub fn set_privilege_level(&mut self, privilege_level: PrivilegeLevel) -> &mut Self {
        self.0.set_bits(13..15, privilege_level.as_u16());
        self
    }

    pub unsafe fn set_stack_index(&mut self, index: u16) -> &mut Self {
        self.0.set_bits(0..3, index + 1);
        self
    }
}
