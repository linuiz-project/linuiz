use crate::interrupts::InterruptCell;
use core::fmt::Write;
use spin::{Mutex, Once};
use uart::{
    Baud, Data, FifoControl, InterruptEnable, LineControl, LineStatus, ModemControl, Uart, address::PortAddress,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("UART loopback integrity check failed")]
    IntegrityCheck,
}

const UART_FIFO_SIZE: usize = 16;

pub struct UartLogger {
    writer: InterruptCell<Mutex<UartWriter>>,
}

// Safety: System UART is not thread-specific in kernel.
unsafe impl Send for UartLogger {}
// Safety: Allows only one instance to be created.
unsafe impl Sync for UartLogger {}

impl UartLogger {
    pub fn init() -> Result<(), Error> {
        crate::interrupts::without(|| {
            static UART_LOGGER: Once<UartLogger> = Once::new();

            UART_LOGGER.try_call_once(|| {
                // Safety: Function invariants provide safety guarantees.
                let mut uart = unsafe {
                    Uart::<PortAddress, Data>::new({
                        #[cfg(target_arch = "x86_64")]
                        {
                            PortAddress::new(0x3F8)
                        }
                    })
                };

                // Bring UART to a known (mostly disabled) state.
                uart.write_line_control(LineControl::empty());
                uart.write_interrupt_enable(InterruptEnable::empty());

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
                uart.write_fifo_control(FifoControl::ENABLE | FifoControl::CLEAR_RX | FifoControl::CLEAR_TX);
                uart.write_modem_control(ModemControl::TERMINAL_READY | ModemControl::OUT_1 | ModemControl::OUT_2);

                Ok(UartLogger { writer: InterruptCell::new(Mutex::new(UartWriter(uart))) })
            })?;

            #[cfg(debug_assertions)]
            log::set_max_level(log::LevelFilter::Trace);
            #[cfg(not(debug_assertions))]
            log::set_max_level(log::LevelFilter::Trace);

            log::set_logger(UART_LOGGER.get().unwrap()).unwrap();

            Ok(())
        })
    }
}

struct UartWriter(Uart<PortAddress, Data>);

impl UartWriter {
    fn wait_for_empty(&self) {
        while !self.0.read_line_status().contains(LineStatus::THR_EMPTY) {
            core::hint::spin_loop();
        }
    }
}

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for (index, c) in s.chars().enumerate() {
            // Wait for the FIFO to empty initially and every 16 bytes written.
            if (index % UART_FIFO_SIZE) == 0 {
                self.wait_for_empty();
            }

            self.0.write_byte(u8::try_from(c).unwrap_or(b'?'));
        }

        Ok(())
    }
}

impl log::Log for UartLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.writer.with(|writer| {
                let mut writer = writer.lock();

                // TODO tell the time?
                let ticks = 1234;

                writeln!(
                    writer,
                    "[#{hwthread_id}][T{ticks}][{level}] {args}",
                    hwthread_id = crate::cpu::get_id(),
                    level = record.level(),
                    args = record.args(),
                )
                .unwrap();
            });
        }
    }

    fn flush(&self) {
        todo!()
    }
}
