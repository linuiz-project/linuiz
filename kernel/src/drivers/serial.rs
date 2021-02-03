#![allow(dead_code)]

use bitflags::bitflags;
use lazy_static::lazy_static;
use libkernel::io::port::{ReadOnlyPort, ReadWritePort, WriteOnlyPort};
use spin::{Mutex, MutexGuard};

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

bitflags! {
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

bitflags! {
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
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialSpeed {
    S115200 = 1,
    S57600 = 2,
    S38400 = 3,
}

#[derive(Debug, Clone, Eq, PartialEq)]
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
    pub unsafe fn init(base: u16, speed: SerialSpeed) -> Self {
        let mut data = ReadWritePort::<u8>::new(base + DATA);
        let mut irq_control = WriteOnlyPort::<u8>::new(base + IRQ_CONTROL);
        let mut fifo_control = WriteOnlyPort::<u8>::new(base + FIFO_CONTROL);
        let mut line_control = WriteOnlyPort::<u8>::new(base + LINE_CONTROL);
        let mut modem_control = WriteOnlyPort::<u8>::new(base + MODEM_CONTROL);
        let line_status = ReadOnlyPort::<u8>::new(base + LINE_STATUS);
        let modem_status = ReadOnlyPort::<u8>::new(base + MODEM_STATUS);
        let scratch = ReadWritePort::<u8>::new(base + SCRATCH);

        // disable irqs
        irq_control.write(0x0);

        // enable DLAB
        line_control.write(LineControlFlags::ENABLE_DLAB.bits());

        // set port speed
        data.write((((speed as u16) >> 8) * 0xFF) as u8);
        data.write(((speed as u16) & 0xFF) as u8);

        // disable DLAB and set data word length to 8 bits, one stop bit
        line_control.write(LineControlFlags::COMMON.bits());

        // enable FIFO, clear queue, with 14b threshold
        fifo_control.write(0xC7);

        // IRQs enabled, RTS/DSR set
        modem_control.write(0x0B);

        // set in loopbkack mode and test serial
        modem_control.write(0x1E);
        data.write(0xAE);
        if data.read() != 0xAE {
            panic!("serial driver is in faulty state (data test failed)");
        }

        // if not faulty, set in normal operation mode
        // (not-loopback with IRQs enabled and OUT#1 and OUT#2 bits enabled)
        modem_control.write(0x0F);

        // enable IRQs
        irq_control.write(0x1);

        Serial {
            data,
            irq_control,
            fifo_control,
            line_control,
            modem_control,
            line_status,
            modem_status,
            scratch,
        }
    }

    /// Checks whether the given LineStatus bit is present.
    pub fn line_status(&mut self, status: LineStatus) -> bool {
        LineStatus::from_bits_truncate(self.line_status.read()).contains(status)
    }

    pub fn write(&mut self, byte: u8) {
        // This ensures we don't overwrite pending data.
        while !self.line_status(LineStatus::TRANSMITTER_EMPTY) {}

        self.data.write(byte);
    }

    pub fn write_buffer(&mut self, buffer: &[u8]) {
        for byte in buffer {
            self.write(*byte);
        }
    }

    pub fn write_string(&mut self, string: &str) {
        for byte in string.bytes() {
            self.write(byte);
        }
    }

    /// Reads the immediate value in the specified port.
    pub fn read_immediate(&mut self) -> u8 {
        self.data.read()
    }

    /// Waits for data to be ready on the data port, and then reads it.
    pub fn read_wait(&mut self) -> u8 {
        while !self.line_status(LineStatus::DATA_RECEIVED) {}

        self.data.read()
    }
}

impl core::fmt::Write for Serial {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_string(string);
        Ok(())
    }
}

lazy_static! {
    static ref SERIAL: Mutex<Serial> =
        Mutex::new(unsafe { Serial::init(COM1, SerialSpeed::S115200) });
}

pub fn serial_line<F>(callback: F)
where
    F: Fn(&mut MutexGuard<Serial>),
{
    // this allows us to semantically lock the serial driver
    //
    // for instance, in case we would like to avoid writing while
    // an interrupt is in progress
    libkernel::instructions::interrupts::without_interrupts(|| {
        callback(&mut SERIAL.lock());
    });
}

#[doc(hidden)]
pub fn __serial_out(args: core::fmt::Arguments) {
    serial_line(|serial| {
        use core::fmt::Write;
        serial.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! serial {
    ($($arg:tt)*) => ($crate::drivers::serial::__serial_out(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serialln {
    () => ($crate::serial!("\n"));
    ($($arg:tt)*) => ($crate::serial!("{}\n", format_args!($($arg)*)));
}
