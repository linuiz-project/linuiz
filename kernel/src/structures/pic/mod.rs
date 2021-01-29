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

impl From<usize> for InterruptOffset {
    fn from(value: usize) -> Self {
        match value {
            0 => InterruptOffset::Timer,
            1 => InterruptOffset::Keyboard,
            2 => InterruptOffset::Cascade,
            3 => InterruptOffset::COM2,
            4 => InterruptOffset::COM1,
            5 => InterruptOffset::LPT2,
            6 => InterruptOffset::FloppyDisk,
            7 => InterruptOffset::LPT1,
            8 => InterruptOffset::CMOSClock,
            9 => InterruptOffset::Peripheral0,
            10 => InterruptOffset::Peripheral1,
            11 => InterruptOffset::Peripheral2,
            12 => InterruptOffset::PS2Mouse,
            13 => InterruptOffset::FPU,
            14 => InterruptOffset::PrimaryATA,
            15 => InterruptOffset::SecondaryATA,
            _ => panic!("invalid interrupt offset, must be 0..15"),
        }
    }
}

impl From<InterruptOffset> for usize {
    fn from(value: InterruptOffset) -> Self {
        match value {
            InterruptOffset::Timer => 0,
            InterruptOffset::Keyboard => 1,
            InterruptOffset::Cascade => 2,
            InterruptOffset::COM2 => 3,
            InterruptOffset::COM1 => 4,
            InterruptOffset::LPT2 => 5,
            InterruptOffset::FloppyDisk => 6,
            InterruptOffset::LPT1 => 7,
            InterruptOffset::CMOSClock => 8,
            InterruptOffset::Peripheral0 => 9,
            InterruptOffset::Peripheral1 => 10,
            InterruptOffset::Peripheral2 => 11,
            InterruptOffset::PS2Mouse => 12,
            InterruptOffset::FPU => 13,
            InterruptOffset::PrimaryATA => 14,
            InterruptOffset::SecondaryATA => 15,
        }
    }
}

lazy_static! {
    static ref PICS: spin::Mutex<ChainedPICs> =
        spin::Mutex::new(unsafe { ChainedPICs::new(PIC_0_OFFSET, PIC_1_OFFSET) });
}

pub fn init() {
    unsafe {
        PICS.lock()
            .init(InterruptLines::TIMER | InterruptLines::CASCADE);
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
