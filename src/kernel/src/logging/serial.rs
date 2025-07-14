use crate::interrupts::InterruptCell;
use core::{fmt::Write, num::NonZero};
use spin::{Mutex, Once};
use uart::{
    Baud, Data, FifoControl, LineControl, LineStatus, ModemControl, Uart, address::PortAddress,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("UART loopback integrity check failed")]
    IntegrityCheck,
}

const UART_FIFO_SIZE: usize = 16;

pub struct Logger(InterruptCell<Mutex<Writer>>);

impl Logger {
    /// Initializes the UART-based serial logging device.
    pub fn init() -> Result<&'static Self, Error> {
        static UART_LOGGER: Once<Logger> = Once::new();

        UART_LOGGER.try_call_once(|| {
            // Safety: Value is >0.
            let port_address = unsafe { NonZero::new_unchecked(0x3F8) };

            let uart = Uart::new_reset({
                // Safety: Function invariants provide safety guarantees.
                #[cfg(target_arch = "x86_64")]
                unsafe {
                    PortAddress::new(port_address)
                }
            });

            // Configure the baud rate (tx/rx speed) to maximum.
            let mut uart = uart.into_dlab_mode();
            uart.set_baud(Baud::B115200);
            let mut uart = uart.into_data_mode();

            // Set character size to 8 bits with no parity.
            uart.write_line_control(LineControl::BITS_8);

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
                return Err(Error::IntegrityCheck);
            }

            // Fully enable UART, with FIFO.
            uart.write_fifo_control(
                FifoControl::ENABLE | FifoControl::CLEAR_RX | FifoControl::CLEAR_TX,
            );
            uart.write_modem_control(
                ModemControl::TERMINAL_READY | ModemControl::OUT_1 | ModemControl::OUT_2,
            );

            b"-SERIAL LOGGER-\n".iter().copied().for_each(|byte| {
                uart.write_byte(byte);
            });

            Ok(Self(InterruptCell::new(Mutex::new(Writer(uart)))))
        })
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            super::with_formatted_log_record(record, |args| {
                self.0.with(|writer| {
                    let mut writer = writer.lock();

                    writer.write_fmt(args).ok();
                });
            });
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}

struct Writer(Uart<PortAddress, Data>);

impl Writer {
    fn wait_for_empty(&mut self) {
        while !self.0.read_line_status().contains(LineStatus::THR_EMPTY) {
            core::hint::spin_loop();
        }
    }
}

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for (index, c) in s.chars().enumerate() {
            // Wait for the FIFO to empty initially and every 16 bytes written.
            if index.is_multiple_of(UART_FIFO_SIZE) {
                self.wait_for_empty();
            }

            self.0.write_byte(u8::try_from(c).unwrap_or(b'?'));
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.wait_for_empty();
        self.0.write_byte(u8::try_from(c).unwrap_or(b'?'));

        Ok(())
    }
}
