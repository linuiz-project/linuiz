use spin::Mutex;
use uart::{Data, Uart};
use crate::interrupts::InterruptCell;

struct UartWriter(Uart<Data>);

impl UartWriter {
    fn write_bytes(&mut self, bytes: impl Iterator<Item = u8>) {
        for (index, byte) in bytes.enumerate() {
            if (index % 14) == 0 {
                while !self.0.read_line_status().contains(uart::LineStatus::TRANSMIT_EMPTY_IDLE) {
                    core::hint::spin_loop();
                }
            } else {
                while !self.0.read_line_status().contains(uart::LineStatus::TRANSMIT_EMPTY) {
                    core::hint::spin_loop();
                }
            }

            self.0.write_data(byte);
        }
    }
}

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if s.is_ascii() {
            self.write_bytes(s.bytes());
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        if c.is_ascii() {
            while !self.0.read_line_status().contains(uart::LineStatus::TRANSMIT_EMPTY_IDLE) {
                core::hint::spin_loop();
            }

            self.0.write_data({
                let mut buffer = [0u8; 4];
                c.encode_utf8(&mut buffer);
                buffer[0]
            });

            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

pub struct Serial(InterruptCell<Mutex<UartWriter>>);

impl Serial {
    /// ### Safety
    ///
    /// This function expects to be called only once per boot cycle.
    pub unsafe fn init() -> Self {
        use uart::{LineControl, ModemControl};

        let mut uart = {
            #[cfg(target_arch = "x86_64")]
            {
                Uart::<Data>::new(uart::COM1)
            }
        };

        // Bring UART to a known state.
        uart.write_line_control(LineControl::empty());
        uart.write_interrupt_enable(uart::InterruptEnable::empty());

        // Configure the baud rate (tx/rx speed).
        let mut uart = uart.configure_mode();
        uart.set_baud(uart::Baud::B115200);
        let mut uart = uart.data_mode();

        // Configure total UART state.
        uart.write_line_control(LineControl {
            bits: uart::DataBits::Eight,
            parity: uart::ParityMode::None,
            extra_stop: false,
            break_signal: false,
        });
        uart.enable_fifo(true, true, false, uart::FifoSize::Fourteen);

        // Test the UART to ensure it's functioning correctly.
        uart.write_model_control(
            ModemControl::REQUEST_TO_SEND
                | ModemControl::AUXILIARY_OUTPUT_1
                | ModemControl::AUXILIARY_OUTPUT_2
                | ModemControl::LOOPBACK_MODE,
        );
        uart.write_data(0x1F);
        assert_eq!(uart.read_data(), 0x1F);

        // Configure modem control for actual UART usage.
        uart.write_model_control(
            ModemControl::TERMINAL_READY
                | ModemControl::REQUEST_TO_SEND
                | ModemControl::AUXILIARY_OUTPUT_1
                | ModemControl::AUXILIARY_OUTPUT_2,
        );

        Self(InterruptCell::new(Mutex::new(UartWriter(uart))))
    }
}

impl log::Log for Serial {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            // TODO tell the time
            let ticks = 0;
            let whole_time = ticks / 1000;
            let frac_time = ticks % 1000;
            self.0.with(|uart| {
                use core::fmt::Write;

                let mut uart = uart.lock();

                uart.write_fmt(format_args!(
                    "[{whole_time:wwidth$}.{frac_time:0fwidth$}][{level}] {args}\n",
                    level = record.level(),
                    args = record.args(),
                    wwidth = 4,
                    fwidth = 3
                ))
                .unwrap();
            });
        }
    }

    fn flush(&self) {}
}
