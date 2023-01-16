#![no_std]
#![allow(dead_code)]

use core::marker::PhantomData;

use bit_field::BitField;
use bitflags::bitflags;

/// Address of the first COM port.
/// This port is VERY likely to be at this address.
#[cfg(target_arch = "x86_64")]
pub const COM1: UartAddress = UartAddress::Io(0x3F8);
/// Address of the second COM port.
/// This port is likely to be at this address.
#[cfg(target_arch = "x86_64")]
pub const COM2: UartAddress = UartAddress::Io(0x2F8);
/// Address of the third COM port.
/// This address is configurable on some BIOSes, so it is not a very reliable port address.
#[cfg(target_arch = "x86_64")]
pub const COM3: UartAddress = UartAddress::Io(0x3E8);
/// Address of the fourth COM port.
/// This address is configurable on some BIOSes, so it is not a very reliable port address.
#[cfg(target_arch = "x86_64")]
pub const COM4: UartAddress = UartAddress::Io(0x2E8);

bitflags! {
    #[repr(transparent)]
    pub struct InterruptEnable : u8 {
        /// Interrupt when received data is available.
        const RECEIVED_DATA = 1 << 0;
        /// Interrupt when the transmit holding register is empty.
        const TRANSMIT_EMPTY = 1 << 1;
        /// Interrupt when the receiver line status register changes.
        const RECEIVE_STATUS = 1 << 2;
        /// Interrupt when the modem status reguster changes.
        const MODEM_STATUS = 1 << 3;
        /// This bit is UART 16750 -specific.
        const SLEEP_MODE = 1 << 4;
        /// This bit is UART 16750 -specific.
        const LOW_POWER = 1 << 5;
        // Bit 6 reserved
        // Bit 7 reserved
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FifoSize {
    Four = 0b01,
    Eight = 0b10,
    Fourteen = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    Five = 0b00,
    Six = 0b01,
    Seven = 0b10,
    Eight = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityMode {
    None = 0b000,
    Odd = 0b001,
    Even = 0b011,
    High = 0b101,
    Low = 0b111,
}

/// Serial port speed, measured in bauds.
#[repr(u16)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Baud {
    B115200 = 1,
    B57600 = 2,
    B38400 = 3,
    B19200 = 6,
    B9600 = 12,
    B4800 = 24,
    B2400 = 48,
    B1200 = 96,
    B300 = 384,
    B50 = 2304,
}

#[repr(C, packed)]
pub struct LineControl {
    pub bits: DataBits,
    pub parity: ParityMode,
    pub extra_stop: bool,
    pub break_signal: bool,
}

impl LineControl {
    #[inline]
    pub const fn empty() -> Self {
        Self { bits: DataBits::Five, parity: ParityMode::None, extra_stop: false, break_signal: false }
    }

    #[inline]
    pub fn as_u8(self) -> u8 {
        *0.set_bits(0..2, self.bits as u8)
            .set_bit(2, self.extra_stop)
            .set_bits(3..6, self.parity as u8)
            .set_bit(6, self.break_signal)
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct ModemControl : u8 {
        const TERMINAL_READY = 1 << 0;
        const REQUEST_TO_SEND = 1 << 1;
        const AUXILIARY_OUTPUT_1 = 1 << 2;
        const AUXILIARY_OUTPUT_2 = 1 << 3;
        const LOOPBACK_MODE = 1 << 4;
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct LineStatus : u8 {
        const DATA_AVAILABLE = 1 << 0;
        const OVERRUN_ERROR = 1 << 1;
        const PARITY_ERROR = 1 << 2;
        const FRAMING_ERROR = 1 << 3;
        const BREAK_INDICATOR = 1 << 4;
        const TRANSMIT_EMPTY = 1 << 5;
        const TRANSMIT_EMPTY_IDLE = 1 << 6;
        const IMPENDING_ERROR = 1 << 7;
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct ModemStatus : u8 {
        const CLEAR_TO_SEND_CHANGED = 1 << 0;
        const DATA_SET_READY_CHANGED= 1 << 1;
        const TRAILING_EDGE_RING_INDICATOR = 1 << 2;
        const CARRIER_DETECT_CHANGE = 1 << 3;
        const CLEAR_TO_SEND = 1 << 4;
        const DATA_SET_READY = 1 << 5;
        const RING_INDICATOR = 1 << 6;
        const CARRIER_DETECT = 1 << 7;
    }
}

pub enum UartAddress {
    Io(u16),
    Mmio(*mut u8),
}

#[repr(usize)]
enum ReadOffset {
    Data0 = 0x0,
    Data1 = 0x1,
    InterruptIdent = 0x2,
    LineControl = 0x3,
    ModemControl = 0x4,
    LineStatus = 0x5,
    ModemStatus = 0x6,
}

#[repr(usize)]
enum WriteOffset {
    Data0 = 0x0,
    Data1 = 0x1,
    FifoControl = 0x2,
    LineControl = 0x3,
    ModemControl = 0x4,
}

pub trait Mode {}
pub struct Data;
impl Mode for Data {}
pub struct Configure;
impl Mode for Configure {}

pub struct Uart<M: Mode>(UartAddress, PhantomData<M>);

/// ### Safety
///
/// Type constructor requires that the inner address be `Send`-able.
unsafe impl<M: Mode> Send for Uart<M> {}

impl<M: Mode> Uart<M> {
    #[inline]
    fn read(&self, offset: ReadOffset) -> u8 {
        // ### Safety: Constructor for `Uart` requires a valid base address.
        unsafe {
            match self.0 {
                UartAddress::Io(port) => {
                    let value: u8;

                    #[cfg(target_arch = "x86_64")]
                    core::arch::asm!("in al, dx", out("al") value, in("dx") port + (offset as u16), options(nostack, nomem, preserves_flags));
                    #[cfg(not(target_arch = "x86_64"))]
                    unimplemented!();

                    value
                }

                UartAddress::Mmio(ptr) => ptr.add(offset as usize).read_volatile(),
            }
        }
    }

    #[inline]
    fn write(&mut self, offset: WriteOffset, value: u8) {
        // ### Safety: Constructor for `Uart` requires a valid base address.
        unsafe {
            match self.0 {
                UartAddress::Io(port) => {
                    #[cfg(target_arch = "x86_64")]
                    core::arch::asm!("out dx, al", in("dx") port + (offset as u16), in("al") value, options(nostack, nomem, preserves_flags));
                    #[cfg(not(target_arch = "x86_64"))]
                    unimplemented!();
                }

                UartAddress::Mmio(ptr) => ptr.add(offset as usize).write_volatile(value),
            }
        }
    }

    // TODO safety
    #[inline]
    pub fn disable_fifo(&mut self) {
        self.write(WriteOffset::FifoControl, 0x0);
    }

    // TODO safety
    #[inline]
    pub fn enable_fifo(
        &mut self,
        clear_rx: bool,
        clear_tx: bool,
        dma_mode_1: bool,
        /* todo enable_64_byte_buffer */ size: FifoSize,
    ) {
        self.write(
            WriteOffset::FifoControl,
            *1.set_bit(1, clear_rx).set_bit(2, clear_tx).set_bit(3, dma_mode_1).set_bits(6..8, size as u8),
        )
    }

    #[inline]
    pub fn read_line_control(&self) -> LineControl {
        let line_control_raw = self.read(ReadOffset::LineControl);
        LineControl {
            bits: match line_control_raw.get_bits(0..2) {
                0b00 => DataBits::Five,
                0b01 => DataBits::Six,
                0b10 => DataBits::Seven,
                0b11 => DataBits::Eight,
                _ => unimplemented!(),
            },
            parity: match line_control_raw.get_bits(3..6) {
                0b000 => ParityMode::None,
                0b001 => ParityMode::Odd,
                0b011 => ParityMode::Even,
                0b101 => ParityMode::High,
                0b111 => ParityMode::High,
                _ => unimplemented!(),
            },
            extra_stop: line_control_raw.get_bit(2),
            break_signal: line_control_raw.get_bit(6),
        }
    }

    #[inline]
    pub fn write_line_control(&mut self, value: LineControl) {
        self.write(WriteOffset::LineControl, value.as_u8());
    }

    #[inline]
    pub fn read_modem_control(&self) -> ModemControl {
        ModemControl::from_bits_truncate(self.read(ReadOffset::ModemControl))
    }

    #[inline]
    pub fn write_model_control(&mut self, value: ModemControl) {
        self.write(WriteOffset::ModemControl, value.bits());
    }

    #[inline]
    pub fn read_line_status(&self) -> LineStatus {
        LineStatus::from_bits_truncate(self.read(ReadOffset::LineStatus))
    }

    #[inline]
    pub fn read_modem_status(&self) -> ModemStatus {
        ModemStatus::from_bits_truncate(self.read(ReadOffset::ModemStatus))
    }
}

impl Uart<Configure> {
    #[inline]
    fn read_divisor_latch(&self) -> u16 {
        ((self.read(ReadOffset::Data1) as u16) << 8) | (self.read(ReadOffset::Data0) as u16)
    }

    #[inline]
    fn write_divisor_latch(&mut self, value: u16) {
        self.write(WriteOffset::Data0, value.to_le() as u8);
        self.write(WriteOffset::Data1, (value.to_le() >> 8) as u8);
    }

    pub fn get_baud(&self) -> Baud {
        match self.read_divisor_latch() {
            1 => Baud::B115200,
            2 => Baud::B57600,
            3 => Baud::B38400,
            6 => Baud::B19200,
            12 => Baud::B9600,
            24 => Baud::B4800,
            48 => Baud::B2400,
            96 => Baud::B1200,
            384 => Baud::B300,
            2304 => Baud::B50,
            _ => unimplemented!(),
        }
    }

    pub fn set_baud(&mut self, baud: Baud) {
        self.write_divisor_latch(baud as u16);
    }

    pub fn data_mode(mut self) -> Uart<Data> {
        // enable DLAB
        self.write(WriteOffset::LineControl, self.read_line_control().as_u8());

        Uart::<Data>(self.0, PhantomData)
    }
}

impl Uart<Data> {
    /// ### Safety
    ///
    /// Provided address must be `Send`-able (i.e. can be used on any CPU core).
    pub unsafe fn new(address: UartAddress) -> Self {
        Self(address, PhantomData)
    }

    #[inline]
    pub fn read_data(&self) -> u8 {
        self.read(ReadOffset::Data0)
    }

    #[inline]
    pub fn write_data(&mut self, data: u8) {
        self.write(WriteOffset::Data0, data);
    }

    #[inline]
    pub fn read_interrupt_enable(&self) -> InterruptEnable {
        InterruptEnable::from_bits_truncate(self.read(ReadOffset::Data1))
    }

    #[inline]
    pub fn write_interrupt_enable(&mut self, value: InterruptEnable) {
        self.write(WriteOffset::Data1, value.bits());
    }

    #[inline]
    pub fn configure_mode(mut self) -> Uart<Configure> {
        // enable DLAB
        self.write(WriteOffset::LineControl, self.read_line_control().as_u8() | (1 << 7));

        Uart::<Configure>(self.0, PhantomData)
    }
}
