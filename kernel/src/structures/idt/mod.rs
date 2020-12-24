mod fault_handlers;
mod interrupt_handlers;
mod interrupt_stack_frame;
mod interrupt_vector;

pub use interrupt_stack_frame::InterruptStackFrame;

use crate::structures::{pic::InterruptOffset, DescriptorTablePointer};
use bitflags::bitflags;
use core::ops::{Index, IndexMut};
use fault_handlers::*;
use interrupt_handlers::*;
use interrupt_vector::*;
use lazy_static::lazy_static;

bitflags! {
    #[repr(transparent)]
    pub struct PageFaultError: u64 {
        const PROTECTION_VIOLATION = 1;
        const CAUSED_BY_WRITE= 1 << 1;
        const USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
    }
}

#[repr(C)]
#[repr(align(16))]
pub struct InterruptDescriptorTable {
    pub divide_error: InterruptVector<InterruptHandler>,
    pub debug: InterruptVector<InterruptHandler>,
    pub non_maskable_interrupt: InterruptVector<InterruptHandler>,
    pub breakpoint: InterruptVector<InterruptHandler>,
    pub overflow: InterruptVector<InterruptHandler>,
    pub bound_range_exceeded: InterruptVector<InterruptHandler>,
    pub invalid_opcode: InterruptVector<InterruptHandler>,
    pub device_not_available: InterruptVector<InterruptHandler>,
    pub double_fault: InterruptVector<DivergingHandlerWithErrCode>,
    coprocessor_segment_overrun: InterruptVector<InterruptHandler>,
    pub invalid_tss: InterruptVector<InterruptHandlerWithErrCode>,
    pub segment_not_present: InterruptVector<InterruptHandlerWithErrCode>,
    pub stack_segment_fault: InterruptVector<InterruptHandlerWithErrCode>,
    pub general_protection_fault: InterruptVector<InterruptHandlerWithErrCode>,
    pub page_fault: InterruptVector<PageFaultHandler>,
    reserved_1: InterruptVector<InterruptHandler>,
    pub x87_floating_point: InterruptVector<InterruptHandler>,
    pub alignment_check: InterruptVector<InterruptHandlerWithErrCode>,
    pub machine_check: InterruptVector<DivergingHandler>,
    pub simd_floating_point: InterruptVector<InterruptHandler>,
    pub virtualization: InterruptVector<InterruptHandler>,
    reserved_2: [InterruptVector<InterruptHandler>; 9],
    pub security_exception: InterruptVector<InterruptHandlerWithErrCode>,
    reserved_3: InterruptVector<InterruptHandler>,
    interrupts: [InterruptVector<InterruptHandler>; 256 - 32],
}

impl InterruptDescriptorTable {
    pub fn new() -> Self {
        Self {
            divide_error: InterruptVector::missing(),
            debug: InterruptVector::missing(),
            non_maskable_interrupt: InterruptVector::missing(),
            breakpoint: InterruptVector::missing(),
            overflow: InterruptVector::missing(),
            bound_range_exceeded: InterruptVector::missing(),
            invalid_opcode: InterruptVector::missing(),
            device_not_available: InterruptVector::missing(),
            double_fault: InterruptVector::missing(),
            coprocessor_segment_overrun: InterruptVector::missing(),
            invalid_tss: InterruptVector::missing(),
            segment_not_present: InterruptVector::missing(),
            stack_segment_fault: InterruptVector::missing(),
            general_protection_fault: InterruptVector::missing(),
            page_fault: InterruptVector::missing(),
            reserved_1: InterruptVector::missing(),
            x87_floating_point: InterruptVector::missing(),
            alignment_check: InterruptVector::missing(),
            machine_check: InterruptVector::missing(),
            simd_floating_point: InterruptVector::missing(),
            virtualization: InterruptVector::missing(),
            reserved_2: [InterruptVector::missing(); 9],
            security_exception: InterruptVector::missing(),
            reserved_3: InterruptVector::missing(),
            interrupts: [InterruptVector::missing(); 256 - 32],
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn load(&'static self) {
        unsafe { self.load_unsafe() }
    }

    unsafe fn load_unsafe(&self) {
        crate::instructions::interrupts::lidt(&self.pointer());
    }

    fn pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (core::mem::size_of::<Self>() - 1) as u16,
        }
    }
}

impl Index<usize> for InterruptDescriptorTable {
    type Output = InterruptVector<InterruptHandler>;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.divide_error,
            1 => &self.debug,
            2 => &self.non_maskable_interrupt,
            3 => &self.breakpoint,
            4 => &self.overflow,
            5 => &self.bound_range_exceeded,
            6 => &self.invalid_opcode,
            7 => &self.device_not_available,
            9 => &self.coprocessor_segment_overrun,
            16 => &self.x87_floating_point,
            19 => &self.simd_floating_point,
            20 => &self.virtualization,
            i @ 32..=255 => &self.interrupts[i - 32],
            i @ 15 | i @ 31 | i @ 21..=29 => panic!("entry {} is reserved", i),
            i @ 8 | i @ 10..=14 | i @ 17 | i @ 30 => {
                panic!("entry {} is an exception with error code", i)
            }
            i @ 18 => panic!("entry {} is a diverging exception (must not return)", i),
            i => panic!("no entry with index {}", i),
        }
    }
}

impl IndexMut<usize> for InterruptDescriptorTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.divide_error,
            1 => &mut self.debug,
            2 => &mut self.non_maskable_interrupt,
            3 => &mut self.breakpoint,
            4 => &mut self.overflow,
            5 => &mut self.bound_range_exceeded,
            6 => &mut self.invalid_opcode,
            7 => &mut self.device_not_available,
            9 => &mut self.coprocessor_segment_overrun,
            16 => &mut self.x87_floating_point,
            19 => &mut self.simd_floating_point,
            20 => &mut self.virtualization,
            i @ 32..=255 => &mut self.interrupts[i - 32],
            i @ 15 | i @ 31 | i @ 21..=29 => panic!("entry {} is reserved", i),
            i @ 8 | i @ 10..=14 | i @ 17 | i @ 30 => {
                panic!("entry {} is an exception with error code", i)
            }
            i @ 18 => panic!("entry {} is a diverging exception (must not return)", i),
            i => panic!("no entry with index {}", i),
        }
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // fault interrupts
        idt.breakpoint.set_handler(breakpoint_handler);
        idt.page_fault.set_handler(page_fault_handler);

        unsafe {
            idt.double_fault.set_handler(double_fault_handler).set_stack_index(crate::structures::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        // regular interrupts
        idt[InterruptOffset::Timer.into()].set_handler(timer_interrupt_handler);

        idt
    };
}

pub fn init() {
    IDT.load();
}
