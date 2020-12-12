/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

use pic8259_simple::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum InterruptOffset {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl Into<u8> for InterruptOffset {
    fn into(self) -> u8 {
        self as u8
    }
}

impl Into<usize> for InterruptOffset {
    fn into(self) -> usize {
        let byte: u8 = self.into();
        usize::from(byte)
    }
}

const fn crate_pics() -> ChainedPics {
    unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }
}

pub static PICS: spin::Mutex<ChainedPics> = spin::Mutex::new(crate_pics());

pub fn init() {
    unsafe {
        PICS.lock().initialize();
    }
}

pub fn end_of_interrupt(offset: InterruptOffset) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(offset.into());
    }
}
