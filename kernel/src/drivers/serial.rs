use crate::io::port::Port;
use lazy_static::lazy_static;
use spin::mutex::{Mutex, MutexGuard};

pub const COM1: u16 = 0x3F8;
pub const LINE_ENABLE_DLAB: u8 = 0x80;

/// COM/Serial port address offsets.
#[repr(u16)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialPort {
    Data = 0x0,
    FIFOControl = 0x2,
    LineControl = 0x3,
    ModemControl = 0x4,
    LineStatus = 0x5,
    ModemStatus = 0x6,
    Scratch = 0x7,
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
    data: Port<u8>,
    fifo_control: Port<u8>,
    line_control: Port<u8>,
    modem_control: Port<u8>,
    line_status: Port<u8>,
    modem_status: Port<u8>,
    scratch: Port<u8>,
}

impl Serial {
    pub unsafe fn init(com: u16, speed: SerialSpeed) -> Self {
        let mut data = Port::<u8>::new(com + (SerialPort::Data as u16));
        let mut fifo_control = Port::<u8>::new(com + (SerialPort::FIFOControl as u16));
        let mut line_control = Port::<u8>::new(com + (SerialPort::LineControl as u16));
        let mut modem_control = Port::<u8>::new(com + (SerialPort::ModemControl as u16));
        let line_status = Port::<u8>::new(com + (SerialPort::LineStatus as u16));
        let modem_status = Port::<u8>::new(com + (SerialPort::ModemStatus as u16));
        let scratch = Port::<u8>::new(com + (SerialPort::Scratch as u16));

        // configure the serial port
        // read https://littleosbook.github.io/#configuring-the-serial-port

        line_control.write(LINE_ENABLE_DLAB);
        data.write((((speed as u16) >> 8) * 0xFF) as u8);
        data.write(((speed as u16) & 0xFF) as u8);

        line_control.write(0x3);

        // enable FIFO, clear queue, with 14b threshold
        fifo_control.write(0xC7);

        // IRQs enabled, RTS/DSR set
        modem_control.write(0x0B);

        Serial {
            data,
            fifo_control,
            line_control,
            modem_control,
            line_status,
            modem_status,
            scratch,
        }
    }

    pub fn write(&mut self, port: SerialPort, byte: u8) {
        // this ensures we don't overwrite pending data
        while !self.is_write_empty() {}

        // write to port
        match port {
            SerialPort::Data => self.data.write(byte),
            SerialPort::FIFOControl => self.fifo_control.write(byte),
            SerialPort::LineControl => self.line_control.write(byte),
            SerialPort::ModemControl => self.modem_control.write(byte),
            SerialPort::LineStatus => self.line_status.write(byte),
            SerialPort::ModemStatus => self.modem_status.write(byte),
            SerialPort::Scratch => self.scratch.write(byte),
        }
    }

    pub fn write_buffer(&mut self, port: SerialPort, buffer: &[u8]) {
        for byte in buffer {
            self.write(port, *byte);
        }
    }

    pub fn write_string(&mut self, port: SerialPort, string: &str) {
        for byte in string.bytes() {
            match byte {
                0x20..=0x7E | b'\n' => self.write(port, byte),
                _ => self.write(port, 0xFE),
            }
        }
    }

    pub fn read(&mut self, port: SerialPort) -> u8 {
        // ensure there's data to be read
        if self.serial_received() {
            // read data from port
            match port {
                SerialPort::Data => self.data.read(),
                SerialPort::FIFOControl => self.fifo_control.read(),
                SerialPort::LineControl => self.line_control.read(),
                SerialPort::ModemControl => self.modem_control.read(),
                SerialPort::LineStatus => self.line_status.read(),
                SerialPort::ModemStatus => self.modem_status.read(),
                SerialPort::Scratch => self.scratch.read(),
            }
        } else {
            0x0
        }
    }

    pub fn is_write_empty(&mut self) -> bool {
        (self.read(SerialPort::LineStatus) & 0x20) == 0x0
    }

    pub fn serial_received(&mut self) -> bool {
        (self.read(SerialPort::LineStatus) & 0x1) > 0x0
    }
}

impl core::fmt::Write for Serial {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_string(SerialPort::Data, string);
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
    callback(&mut SERIAL.lock());
}

#[doc(hidden)]
pub fn __print(args: core::fmt::Arguments) {
    safe_lock(|serial| {
        use core::fmt::Write;
        serial.write_fmt(args).unwrap();
    })
}

#[macro_export]
macro_rules! serial {
    ($($arg:tt)*) => ($crate::drivers::serial::__print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serialln {
    () => ($crate::print!('\n'));
    ($($arg:tt)*) => ($crate::serial!("{}\n", format_args!($($arg)*)));
}
