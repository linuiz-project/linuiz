/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pic8259;

use crate::io::port::WriteOnlyPort;
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

pub const MAXIMUM_TICK_RATE: u32 = 1193182;

static CHAINED_PICS: spin::Mutex<ChainedPICs> =
    spin::Mutex::new(unsafe { ChainedPICs::new(InterruptOffset::BASE, InterruptOffset::BASE + 8) });

pub fn enable() {
    unsafe {
        CHAINED_PICS
            .lock()
            .init(InterruptLines::TIMER | InterruptLines::CASCADE);
    }
}

pub fn end_of_interrupt(offset: InterruptOffset) {
    unsafe {
        CHAINED_PICS.lock().end_of_interrupt(offset as u8);
    }
}

pub unsafe fn disable() {
    CHAINED_PICS.lock().init(InterruptLines::empty())
}

pub fn set_timer_freq(hz: u32) {
    const DATA0: u16 = 0x40;
    const COMMAND: u16 = 0x43;

    let mut command = unsafe { WriteOnlyPort::<u8>::new(COMMAND) };
    let mut data0 = unsafe { WriteOnlyPort::<u8>::new(DATA0) };

    let divisor = MAXIMUM_TICK_RATE / hz;
    command.write(0x36);
    data0.write((divisor & 0xFF) as u8);
    data0.write((divisor >> 8) as u8);
}
