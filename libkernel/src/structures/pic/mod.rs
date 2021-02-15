/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pic8259;

use lazy_static::lazy_static;
use pic8259::{ChainedPICs, InterruptLines};
use spin;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptOffset {
    Timer = Self::BASE,
    Keyboard,
    Cascade,
    COM2,
    COM1,
    LPT2,
    FloppyDisk,
    SpuriousMaster,
    CMOSClock,
    Peripheral0,
    Peripheral1,
    Peripheral2,
    PS2Mouse,
    FPU,
    PrimaryATA,
    SpuriousSlave,
}

impl InterruptOffset {
    pub const BASE: u8 = 32;

    pub const fn base_offset(offset: u8) -> Self {
        match offset {
            0 => InterruptOffset::Timer,
            1 => InterruptOffset::Keyboard,
            2 => InterruptOffset::Cascade,
            3 => InterruptOffset::COM2,
            4 => InterruptOffset::COM1,
            5 => InterruptOffset::LPT2,
            6 => InterruptOffset::FloppyDisk,
            7 => InterruptOffset::SpuriousMaster,
            8 => InterruptOffset::CMOSClock,
            9 => InterruptOffset::Peripheral0,
            10 => InterruptOffset::Peripheral1,
            11 => InterruptOffset::Peripheral2,
            12 => InterruptOffset::PS2Mouse,
            13 => InterruptOffset::FPU,
            14 => InterruptOffset::PrimaryATA,
            15 => InterruptOffset::SpuriousSlave,
            _ => panic!("invalid interrupt offset, must be 0..=15",),
        }
    }

    pub const fn without_base(self) -> u8 {
        (self as u8) - Self::BASE
    }
}

lazy_static! {
    static ref PICS: spin::Mutex<ChainedPICs> = spin::Mutex::new(unsafe {
        ChainedPICs::new(InterruptOffset::BASE, InterruptOffset::BASE + 8)
    });
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

pub unsafe fn disable() {
    PICS.lock().init(InterruptLines::empty())
}
