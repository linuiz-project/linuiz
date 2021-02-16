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
    DummyAPIC = 192,
}

impl InterruptOffset {
    pub const BASE: u8 = 32;

    pub fn from_u8(value: u8) -> Self {
        unsafe { core::mem::transmute(value) }
    }

    pub const fn as_usize(self) -> usize {
        self as usize
    }

    pub const fn as_usize_no_base(self) -> usize {
        (self as usize) - (Self::BASE as usize)
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
