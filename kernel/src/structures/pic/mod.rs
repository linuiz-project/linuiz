/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pic8259;

use core::convert::TryFrom;
use lazy_static::lazy_static;
use pic8259::ChainedPICs;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum PICInterrupt {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl TryFrom<u8> for PICInterrupt {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let deoffset_value = value - PIC_1_OFFSET;
        match deoffset_value {
            0 => Ok(PICInterrupt::Timer),
            1 => Ok(PICInterrupt::Keyboard),
            _ => Err(()),
        }
    }
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

lazy_static! {
    static ref PICS: spin::Mutex<ChainedPICs> =
        spin::Mutex::new(unsafe { ChainedPICs::new(PIC_1_OFFSET, PIC_2_OFFSET) });
}

pub fn init() {
    unsafe {
        PICS.lock().init();
    }
}

pub fn end_of_interrupt(offset: PICInterrupt) {
    unsafe {
        PICS.lock().end_of_interrupt(offset.into());
    }
}

pub fn handles_interrupt(interrupt_id: u8) -> bool {
    PICS.lock().handles_interrupt(interrupt_id)
}
