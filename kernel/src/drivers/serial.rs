use crate::io::port::Port;
use bitflags::bitflags;
use lazy_static::lazy_static;
use spin::mutex::{Mutex, MutexGuard};

pub const COM1: u16 = 0x3F8;
pub const LINE_ENABLE_DLAB: u8 = 0x80;

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

/// COM/Serial port address offsets.
#[repr(u16)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SerialPort {
    Data = 0x0,
    IRQControl = 0x1,
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
    irq_control: Port<u8>,
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
        let mut irq_control = Port::<u8>::new(com + (SerialPort::IRQControl as u16));
        let mut fifo_control = Port::<u8>::new(com + (SerialPort::FIFOControl as u16));
        let mut line_control = Port::<u8>::new(com + (SerialPort::LineControl as u16));
        let mut modem_control = Port::<u8>::new(com + (SerialPort::ModemControl as u16));
        let line_status = Port::<u8>::new(com + (SerialPort::LineStatus as u16));
        let modem_status = Port::<u8>::new(com + (SerialPort::ModemStatus as u16));
        let scratch = Port::<u8>::new(com + (SerialPort::Scratch as u16));

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

    fn get_port(&mut self, port: SerialPort) -> &mut Port<u8> {
        match port {
            SerialPort::Data => &mut self.data,
            SerialPort::IRQControl => &mut self.irq_control,
            SerialPort::FIFOControl => &mut self.fifo_control,
            SerialPort::LineControl => &mut self.line_control,
            SerialPort::ModemControl => &mut self.modem_control,
            SerialPort::LineStatus => &mut self.line_status,
            SerialPort::ModemStatus => &mut self.modem_status,
            SerialPort::Scratch => &mut self.scratch,
        }
    }

    pub fn write(&mut self, port: SerialPort, byte: u8) {
        // this ensures we don't overwrite pending data
        while !self.line_status(LineStatus::TRANSMITTER_EMPTY) {}

        // write to port
        self.get_port(port).write(byte);
    }

    pub fn write_buffer(&mut self, buffer: &[u8]) {
        for byte in buffer {
            self.write(SerialPort::Data, *byte);
        }
    }

    pub fn write_string(&mut self, string: &str) {
        for byte in string.bytes() {
            self.write(SerialPort::Data, byte)
        }
    }

    pub fn read_immediate(&mut self, port: SerialPort) -> u8 {
        self.get_port(port).read()
    }

    pub fn read_wait(&mut self, port: SerialPort) -> u8 {
        while !self.line_status(LineStatus::DATA_RECEIVED) {}

        self.get_port(port).read()
    }

    pub fn line_status(&mut self, status: LineStatus) -> bool {
        match LineStatus::from_bits(self.read_immediate(SerialPort::LineStatus)) {
            Some(line_status) => line_status.contains(status),
            None => panic!("failed to parse line status"),
        }
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
