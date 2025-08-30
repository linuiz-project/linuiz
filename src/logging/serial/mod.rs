use crate::util::sync::{Mutex, Once};
use core::num::NonZero;
use uart::{InterruptEnable, LineStatus, address::PortAddress};

#[cfg(target_arch = "x86_64")]
type UartAddress = uart::address::PortAddress;
#[cfg(target_arch = "riscv64")]
type UartAddress = uart::address::MmioAddress;
type Uart = uart::Uart<UartAddress, uart::Data>;

const UART_FIFO_SIZE: usize = 16;
const_assert!(UART_FIFO_SIZE.is_power_of_two());

static UART: Once<Mutex<Uart>> = Once::new();

/// ## Safety
///
/// - `address` must be a valid serial address pointing to a UART 16550 device.
/// - `address` must not be read from or written to by another context.
fn configure_uart(address: UartAddress, buffered: bool) {
    UART.try_call_once(|| {
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
            return Err(());
        }

        // Conditionally enable the `TRANSMIT_EMPTY` interrupt.
        if buffered {
            uart.write_interrupt_enable(InterruptEnable::TRANSMIT_EMPTY);
        } else {
            uart.write_interrupt_enable(InterruptEnable::empty());
        }

        uart.write_modem_control(ModemControl::TERMINAL_READY | ModemControl::OUT_2);

        b"\n-SERIAL LOGGER-\n".iter().for_each(|byte| {
            uart.write_byte(*byte);
        });

        Ok(Mutex::new(uart))
    })
    .ok();
}

type UartStringBuffer = heapless::String<0x2000>;

fn with_buffered_uart(func: impl FnOnce(&'static mut UartStringBuffer, &'static mut Uart)) {
    let Some(uart) = UART.get() else {
        return;
    };

    uart.with_lock(|uart| {
        static WRITE_BUFFER: Mutex<UartStringBuffer> = Mutex::new(UartStringBuffer::new());

        WRITE_BUFFER.with_lock(|write_buffer| {
            func(write_buffer, uart);
        });
    });
}

pub struct Logger;

impl Logger {
    /// Initializes the UART-based serial logging device.
    pub fn init() -> &'static Self {
        let address = {
            cfg_select! {
                target_arch = "x86_64" => {
                    // TODO allow specifying the serial port in the kernel parameters?
                    let port_address = NonZero::<u16>::new(0x3F8).unwrap();
                    // Safety: 0x3F8 is *very likely* to be the correct serial port; even
                    //         if not, there's no way to check.
                    unsafe { PortAddress::new(port_address) }
                }

                _ => { unimplemented!() }
            }
        };

        configure_uart(address, false);

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

        super::with_formatted_log_record(record, |args| {
            with_buffered_uart(|string_buffer, uart| {
                core::fmt::Write::write_fmt(string_buffer, args).ok();

                string_buffer
                    .chars()
                    .enumerate()
                    .map(|(i, c)| (i, u8::try_from(c).unwrap_or(b'?')))
                    .for_each(|(iteration, byte)| {
                        // If we're at the beginning or we've iterated one FIFO length ...
                        if (iteration & UART_FIFO_SIZE) == 0 {
                            // ... then wait for the transmit buffer to empty.
                            while !uart.read_line_status().contains(LineStatus::THR_EMPTY) {
                                core::hint::spin_loop();
                            }
                        }

                        uart.write_byte(byte);
                    });

                string_buffer.clear();
            });
        });
    }

    fn flush(&self) {
        unimplemented!()
    }
}
