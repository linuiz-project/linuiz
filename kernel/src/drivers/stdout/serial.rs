#![allow(dead_code)]

use liblz::io::port::{ReadOnlyPort, ReadWritePort, WriteOnlyPort};

/// Address of the first COM port.
/// This port is VERY likely to be at this address.
pub const COM1: u16 = 0x3F8;
/// Address of the second COM port.
/// This port is likely to be at this address.
pub const COM2: u16 = 0x2F8;
/// Address of the third COM port.
/// This address is configurable on some BIOSes, so it is not a very reliable port address.
pub const COM3: u16 = 0x3E8;
/// Address of the fourth COM port.
/// This address is configurable on some BIOSes, so it is not a very reliable port address.
pub const COM4: u16 = 0x2E8;

/// Address offset of the data port.
pub const DATA: u16 = 0x0;
/// Address offset of the interrupt enable port.
pub const IRQ_CONTROL: u16 = 0x1;
/// Address offset of the FIFO control port.
pub const FIFO_CONTROL: u16 = 0x2;
/// Address offset of the line control port.
pub const LINE_CONTROL: u16 = 0x3;
/// Address offset of the modem control port.
pub const MODEM_CONTROL: u16 = 0x4;
/// Address offset of the line status port.
pub const LINE_STATUS: u16 = 0x5;
/// Address offset of the modem status port.
pub const MODEM_STATUS: u16 = 0x6;
/// Address offset of the scratch port.
pub const SCRATCH: u16 = 0x7;

bitflags::bitflags! {
    pub struct LineStatus : u8 {
        const DATA_RECEIVED = 1 << 0;
        const OVERRUN_ERROR = 1 << 1;
        const PARITY_ERROR = 1 << 2;
        const FRAMING_ERROR = 1 << 3;
        const BREAK_INDICATOR = 1 << 4;
        const TRANSMITTER_HOLDING_REGISTER_EMPTY = 1 << 5;
        const TRANSMITTER_EMPTY = 1 << 6;
        const IMPENDING_ERROR = 1 << 7;
    }
}

bitflags::bitflags! {
    pub struct LineControlFlags : u8 {
        const DATA_0 = 1 << 0;
        const DATA_1 = 1 << 1;
        const STOP = 1 << 2;
        const PARITY_0 = 1 << 3;
        const PARITY_1 = 1 << 4;
        const PARITY_2 = 1 << 5;
        const ENABLE_DLAB = 1 << 7;

        /// Common bits for line control register
        const COMMON = Self::DATA_0.bits() | Self::DATA_1.bits() | Self::STOP.bits();
    }
}

/// Serial port speed, measured in bauds.
#[repr(u16)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialSpeed {
    S115200 = 1,
    S57600 = 2,
    S38400 = 3,
}

pub struct Serial {
    data: ReadWritePort<u8>,
    irq_control: WriteOnlyPort<u8>,
    fifo_control: WriteOnlyPort<u8>,
    line_control: WriteOnlyPort<u8>,
    modem_control: WriteOnlyPort<u8>,
    line_status: ReadOnlyPort<u8>,
    modem_status: ReadOnlyPort<u8>,
    scratch: ReadWritePort<u8>,
}

impl Serial {
    pub const fn new(base: u16) -> Self {
        unsafe {
            Self {
                data: ReadWritePort::<u8>::new(base + DATA),
                irq_control: WriteOnlyPort::<u8>::new(base + IRQ_CONTROL),
                fifo_control: WriteOnlyPort::<u8>::new(base + FIFO_CONTROL),
                line_control: WriteOnlyPort::<u8>::new(base + LINE_CONTROL),
                modem_control: WriteOnlyPort::<u8>::new(base + MODEM_CONTROL),
                line_status: ReadOnlyPort::<u8>::new(base + LINE_STATUS),
                modem_status: ReadOnlyPort::<u8>::new(base + MODEM_STATUS),
                scratch: ReadWritePort::<u8>::new(base + SCRATCH),
            }
        }
    }

    pub unsafe fn init(&mut self, speed: SerialSpeed) {
        // disable irqs
        self.irq_control.write(0x0);

        // enable DLAB
        self.line_control
            .write(LineControlFlags::ENABLE_DLAB.bits());

        // set port speed
        self.data.write((((speed as u16) >> 8) * 0xFF) as u8);
        self.data.write(((speed as u16) & 0xFF) as u8);

        // disable DLAB and set data word length to 8 bits, one stop bit
        self.line_control.write(LineControlFlags::COMMON.bits());

        // enable FIFO, clear queue, with 14b threshold
        self.fifo_control.write(0xC7);

        // IRQs enabled, RTS/DSR set
        self.modem_control.write(0x0B);

        // set in loopbkack mode and test serial
        self.modem_control.write(0x1E);
        self.data.write(0xAE);

        assert_eq!(
            self.data.read(),
            0xAE,
            "serial driver is in faulty state (data test failed)"
        );

        // if not faulty, set in normal operation mode
        // (not-loopback with IRQs enabled and OUT#1 and OUT#2 bits enabled)
        self.modem_control.write(0x0F);

        // enable IRQs
        self.irq_control.write(0x1);
    }

    /// Checks whether the given LineStatus bit is present.
    pub fn line_status(&mut self, status: LineStatus) -> bool {
        LineStatus::from_bits_truncate(self.line_status.read()).contains(status)
    }

    pub unsafe fn write_raw(&mut self, byte: u8) {
        self.data.write(byte);
    }

    pub fn write(&mut self, byte: u8) {
        // This ensures we don't overwrite pending data.
        //
        // REMARK: This does not enure we don't overwrite data asychronously.
        while !self.line_status(LineStatus::TRANSMITTER_EMPTY) {}

        self.data.write(byte);
    }

    pub fn write_str(&mut self, string: &str) {
        for byte in string.bytes() {
            self.write(byte);
        }
    }

    /// Waits for data to be ready on the data port, and then reads it.
    pub fn read(&mut self) -> u8 {
        while !self.line_status(LineStatus::DATA_RECEIVED) {}

        self.data.read()
    }
}

impl core::fmt::Write for Serial {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        if string.is_ascii() {
            self.write_str(string);
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        if c.is_ascii() {
            self.write(c as u8);
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}
