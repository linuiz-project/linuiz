use crate::io::port::{ReadOnlyPort, ReadWritePort, WriteOnlyPort};
use bitflags::bitflags;
use lazy_static::lazy_static;
use spin::mutex::{Mutex, MutexGuard};

pub const COM1: u16 = 0x3F8;
pub const LINE_ENABLE_DLAB: u8 = 0x80;

pub const DATA: u16 = 0x0;
pub const IRQ_CONTROL: u16 = 0x1;
pub const FIFO_CONTROL: u16 = 0x2;
pub const LINE_CONTROL: u16 = 0x3;
pub const MODEM_CONTROL: u16 = 0x4;
pub const LINE_STATUS: u16 = 0x5;
pub const MODEM_STATUS: u16 = 0x6;
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

        // configure the serial port
        // read https://littleosbook.github.io/#configuring-the-serial-port

        // disable irqs
        irq_control.write(0x0);

        // enable DLAB
        line_control.write(LINE_ENABLE_DLAB);

        // set port speed
        data.write((((speed as u16) >> 8) * 0xFF) as u8);
        data.write(((speed as u16) & 0xFF) as u8);

        // disable DLAB and set data word length to 8 bits
        line_control.write(0x3);

        // enable FIFO, clear queue, with 14b threshold
        fifo_control.write(0xC7);

        // IRQs enabled, RTS/DSR set
        modem_control.write(0x0B);

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
        match LineStatus::from_bits(self.line_status.read()) {
            Some(line_status) => line_status.contains(status),
            None => panic!("failed to parse line status"),
        }
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

pub fn safe_lock<F>(callback: F)
where
    F: Fn(&mut MutexGuard<Serial>),
{
    // this allows us to semantically lock the serial driver
    //
    // for instance, in case we would like to avoid writing while
    // an interrupt is in progress
    //x86_64::instructions::interrupts::without_interrupts(|| {
    callback(&mut SERIAL.lock());
    //});
}

#[doc(hidden)]
pub fn __serial_out(args: core::fmt::Arguments) {
    safe_lock(|serial| {
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
