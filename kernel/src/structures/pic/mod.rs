/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pic8259;

use lazy_static::lazy_static;
use pic8259::{ChainedPICs, InterruptLines};
use spin;

pub const PIC_0_OFFSET: u8 = 32;
pub const PIC_1_OFFSET: u8 = PIC_0_OFFSET + 8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptOffset {
    Timer = PIC_0_OFFSET,
    Keyboard,
    Cascade,
    COM2,
    COM1,
    LPT2,
    FloppyDisk,
    LPT1,
    CMOSClock,
    Peripheral0,
    Peripheral1,
    Peripheral2,
    PS2Mouse,
    FPU,
    PrimaryATA,
    SecondaryATA,
}

lazy_static! {
    static ref PICS: spin::Mutex<ChainedPICs> =
        spin::Mutex::new(unsafe { ChainedPICs::new(PIC_0_OFFSET, PIC_1_OFFSET) });
}

pub fn init() {
    unsafe {
        PICS.lock()
            .init(InterruptLines::TIMER | InterruptLines::SLAVE);
    }
}

pub fn end_of_interrupt(offset: InterruptOffset) {
    unsafe {
        PICS.lock().end_of_interrupt(offset as u8);
    }
}

pub fn handles_interrupt(offset: InterruptOffset) -> bool {
    PICS.lock().handles_interrupt(offset as u8)
}
