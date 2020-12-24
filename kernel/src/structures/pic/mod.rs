/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pic8259;

use lazy_static::lazy_static;
use pic8259::ChainedPICs;
use spin;

use super::idt::InterruptType;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum PICInterrupt {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl Into<u8> for PICInterrupt {
    fn into(self) -> u8 {
        self as u8
    }
}

impl Into<usize> for PICInterrupt {
    fn into(self) -> usize {
        self as usize
    }
}

impl Into<InterruptType> for PICInterrupt {
    fn into(self) -> InterruptType {
        InterruptType::Generic(self.into())
    }
}

lazy_static! {
    pub static ref PICS: spin::Mutex<ChainedPICs> =
        spin::Mutex::new(unsafe { ChainedPICs::new(PIC_1_OFFSET, PIC_2_OFFSET) });
}

pub fn init() {
    unsafe {
        PICS.lock().initialize();
    }
}

pub fn end_of_interrupt(offset: PICInterrupt) {
    unsafe {
        PICS.lock().end_of_interrupt(offset.into());
    }
}
