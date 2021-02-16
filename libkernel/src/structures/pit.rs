#![allow(dead_code)]

use crate::io::port::WriteOnlyPort;

const DATA0: u16 = 0x40;
const DATA1: u16 = 0x41;
const DATA2: u16 = 0x42;
const COMMAND: u16 = 0x43;

pub const MAXIMUM_TICK_RATE: u32 = 1193182;

pub fn configure_hz(hz: u32) {
    let mut command = unsafe { WriteOnlyPort::<u8>::new(COMMAND) };
    let mut data0 = unsafe { WriteOnlyPort::<u8>::new(DATA0) };

    let divisor = MAXIMUM_TICK_RATE / hz;
    command.write(0x36);
    data0.write((divisor & 0xFF) as u8);
    data0.write((divisor >> 8) as u8);
}
