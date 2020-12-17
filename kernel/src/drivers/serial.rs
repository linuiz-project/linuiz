use lazy_static::lazy_static;
use spin::mutex::{Mutex, MutexGuard};
use x86_64::instructions::port::Port;

pub const COM1: u16 = 0x3F8;
pub const LINE_ENABLE_DLAB: u8 = 0x80;

#[repr(u16)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialPort {
    DataPort = 0x0,
    FIFOCommandPort = 0x2,
    LineCommandPort = 0x3,
    ModemCommandPort = 0x4,
    LineStatusPort = 0x5,
}

#[repr(u16)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialSpeed {
    S115200 = 1,
    S57600 = 2,
    S38400 = 3,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Serial {
    data_port: Port<u8>,
    fifo_port: Port<u8>,
    line_port: Port<u8>,
    modem_port: Port<u8>,
    status_port: Port<u8>,
}

impl Serial {
    pub unsafe fn init(com: u16, speed: SerialSpeed) -> Self {
        let mut data_port = Port::<u8>::new(com + (SerialPort::DataPort as u16));
        let mut fifo_port = Port::<u8>::new(com + (SerialPort::FIFOCommandPort as u16));
        let mut line_port = Port::<u8>::new(com + (SerialPort::LineCommandPort as u16));
        let mut modem_port = Port::<u8>::new(com + (SerialPort::ModemCommandPort as u16));
        let status_port = Port::<u8>::new(com + (SerialPort::LineStatusPort as u16));

        // configure the serial port
        // read https://littleosbook.github.io/#configuring-the-serial-port

        line_port.write(LINE_ENABLE_DLAB);
        data_port.write((((speed as u16) >> 8) * 0xFF) as u8);
        data_port.write(((speed as u16) & 0xFF) as u8);

        line_port.write(0x3);

        // enable FIFO, clear queue, with 14b threshold
        fifo_port.write(0xC7);

        // IRQs enabled, RTS/DSR set
        modem_port.write(0x0B);

        Serial {
            data_port,
            fifo_port,
            line_port,
            modem_port,
            status_port,
        }
    }

    pub fn write(&mut self, port: SerialPort, byte: u8) {
        unsafe {
            match port {
                SerialPort::DataPort => self.data_port.write(byte),
                SerialPort::FIFOCommandPort => self.fifo_port.write(byte),
                SerialPort::LineCommandPort => self.line_port.write(byte),
                SerialPort::ModemCommandPort => self.modem_port.write(byte),
                SerialPort::LineStatusPort => self.status_port.write(byte),
            }
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
        unsafe {
            match port {
                SerialPort::DataPort => self.data_port.read(),
                SerialPort::FIFOCommandPort => self.fifo_port.read(),
                SerialPort::LineCommandPort => self.line_port.read(),
                SerialPort::ModemCommandPort => self.modem_port.read(),
                SerialPort::LineStatusPort => self.status_port.read(),
            }
        }
    }

    pub fn is_fifo_empty(&mut self) -> bool {
        (self.read(SerialPort::LineStatusPort) & 0x20) == 0x0
    }

    pub fn serial_received(&mut self) -> bool {
        (self.read(SerialPort::LineStatusPort) & 0x1) == 0x0
    }
}

impl core::fmt::Write for Serial {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_string(SerialPort::DataPort, string);
        Ok(())
    }
}

#[doc(hidden)]
pub fn __print(args: core::fmt::Arguments) {
    safe_lock(|serial| {
        use core::fmt::Write;
        serial.write_fmt(args).unwrap();
    })
}

#[macro_export]
macro_rules! write {
    ($($arg:tt)*) => ($crate::drivers::serial::__print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! writeln {
    () => ($crate::print!('\n'));
    ($($arg:tt)*) => ($crate::write!("{}\n", format_args!($($arg)*)));
}
