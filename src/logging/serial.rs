use crate::util::sync::{Mutex, Once};
use core::{ascii::Char as AsciiChar, num::NonZero};
use uart::{InterruptEnable, LineStatus, address::PortAddress};

#[cfg(target_arch = "x86_64")]
type UartAddress = uart::address::PortAddress;
#[cfg(target_arch = "riscv64")]
type UartAddress = uart::address::MmioAddress;
type Uart = uart::Uart<UartAddress, uart::Data>;

static UART: Once<Option<Mutex<UartWriter>>> = Once::new();

pub struct Logger;

impl Logger {
    /// Initializes the UART-based serial logging device.
    pub fn init() -> &'static Self {
        let address = {
            cfg_select! {
                target_arch = "x86_64" => {
                    let port_address = NonZero::<u16>::new(0x3F8).unwrap();
                    // Safety: 0x3F8 is *very likely* to be the correct serial port; even
                    //         if not, there's no way to check.
                    unsafe { PortAddress::new(port_address) }
                }

                _ => { unimplemented!() }
            }
        };

        // TODO Allow specifying UART parameters via kernel's cmdline.
        UART.call_once(|| UartWriter::new(address, false, 1).map(Mutex::new));

        &Self
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let Some(uart) = UART.get().unwrap() else {
            return;
        };

        super::with_formatted_log_record(record, |args| {
            uart.with_lock(|uart| {
                core::fmt::Write::write_fmt(uart, args).ok();
            });
        });
    }

    fn flush(&self) {
        unimplemented!()
    }
}

struct UartWriter {
    uart: Uart,
    fifo_size: usize,
    chars_written: usize,
}

impl UartWriter {
    /// ## Safety
    ///
    /// - `address` must be a valid serial address pointing to a UART 16550
    ///   device.
    /// - `address` must not be read from or written to by another context.
    fn new(address: UartAddress, buffered: bool, fifo_size: usize) -> Option<Self> {
        use uart::{Baud, FifoControl, LineControl, ModemControl};

        // Safety: Caller is required to maintain safety invariants.
        let uart = unsafe { Uart::new(address) };

        // Configure the baud rate (tx/rx speed) to maximum.
        let mut uart = uart.into_dlab_mode();
        uart.set_baud(Baud::B115200);
        let mut uart = uart.into_data_mode();

        // Set character size to 8 bits with no parity.
        uart.write_line_control(LineControl::BITS_8);

        // Fully enable UART, with FIFO.
        uart.write_fifo_control(
            FifoControl::ENABLE | FifoControl::CLEAR_RX | FifoControl::CLEAR_TX,
        );

        // Configure UART into loopback mode to test it.
        uart.write_modem_control(
            ModemControl::REQUEST_TO_SEND
                | ModemControl::OUT_1
                | ModemControl::OUT_2
                | ModemControl::LOOPBACK_MODE,
        );

        // Test the UART to ensure it's functioning correctly.
        uart.write_byte(0x1F);
        if uart.read_byte() != 0x1F {
            return None;
        }

        // Conditionally enable the `TRANSMIT_EMPTY` interrupt.
        if buffered {
            uart.write_interrupt_enable(InterruptEnable::TRANSMIT_EMPTY);
        } else {
            uart.write_interrupt_enable(InterruptEnable::empty());
        }

        uart.write_modem_control(ModemControl::TERMINAL_READY | ModemControl::OUT_2);

        let mut uart_writer = UartWriter {
            uart,
            fifo_size,
            chars_written: 0,
        };

        core::fmt::Write::write_str(&mut uart_writer, "\n-SERIAL LOGGER-\n").ok();

        Some(uart_writer)
    }

    fn is_fifo_maybe_full(&self) -> bool {
        (self.chars_written & self.fifo_size) == 0
    }

    fn write_byte(&mut self, b: u8) {
        // If we're at the beginning or we've iterated one FIFO length ...
        if self.is_fifo_maybe_full() {
            // ... then wait for the transmit buffer to empty.
            while !self.uart.read_line_status().contains(LineStatus::THR_EMPTY) {
                core::hint::spin_loop();
            }
        }

        self.uart.write_byte(b);
        self.chars_written += 1;
    }

    fn write_char(&mut self, c: char) {
        let c_ascii = c.as_ascii().unwrap_or(AsciiChar::QuestionMark);
        let b = c_ascii.to_u8();

        self.write_byte(b);
    }
}

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c| self.write_char(c));

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.write_char(c);

        Ok(())
    }
}
