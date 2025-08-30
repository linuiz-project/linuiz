#[cfg(feature = "buffered_uart")]
mod buffered;
#[cfg(feature = "buffered_uart")]
pub use buffered::Logger;

#[cfg(not(feature = "buffered_uart"))]
mod unbuffered;
#[cfg(not(feature = "buffered_uart"))]
pub use unbuffered::Logger;

use crate::util::sync::{Mutex, Once};

#[cfg(target_arch = "x86_64")]
type UartAddress = uart::address::PortAddress;
#[cfg(target_arch = "riscv64")]
type UartAddress = uart::address::MmioAddress;
type Uart = uart::Uart<UartAddress, uart::Data>;

static UART: Once<Mutex<Uart>> = Once::new();

/// ## Safety
///
/// - `address` must be a valid serial address pointing to a UART 16550 device.
/// - `address` must not be read from or written to by another context.
pub fn configure_uart(address: UartAddress) {
    UART.call_once(|| {
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
        uart.write_modem_control(
            ModemControl::TERMINAL_READY | ModemControl::OUT_1 | ModemControl::OUT_2,
        );

        b"\n-SERIAL LOGGER-\n".iter().for_each(|byte| {
            uart.write_byte(*byte);
        });

        Mutex::new(uart)
    });
}

fn with_uart(func: impl FnOnce(&'static mut Uart)) {
    UART.get().unwrap().with_lock(func);
}
