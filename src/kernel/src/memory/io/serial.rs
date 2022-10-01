use uart::{Data, Uart};

pub struct SerialWriter {
    pending: spin::Once<crossbeam::queue::ArrayQueue<[u8; 14]>>,
    uart: spin::Mutex<uart::Uart<uart::Data>>,
}

impl SerialWriter {
    /// SAFETY: This function expects to be called only once per boot cycle.
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

        Self { pending: spin::Once::new(), uart: spin::Mutex::new(uart) }
    }

    fn write_bytes(uart: &mut spin::MutexGuard<uart::Uart<uart::Data>>, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            if (index % 14) == 0 {
                while !uart.read_line_status().contains(uart::LineStatus::TRANSMIT_EMPTY_IDLE) {
                    core::hint::spin_loop();
                }
            } else {
                while !uart.read_line_status().contains(uart::LineStatus::TRANSMIT_EMPTY) {
                    core::hint::spin_loop();
                }
            }

            uart.write_data(*byte);
        }
    }

    /// SAFETY: Function must not be called from within an interrupted context.
    pub unsafe fn into_queued(&self) {
        libarch::interrupts::without(|| {
            self.pending.call_once(|| crossbeam::queue::ArrayQueue::new(256));
            let mut uart = self.uart.lock();
            uart.write_interrupt_enable(uart::InterruptEnable::TRANSMIT_EMPTY);
        });
    }

    /// SAFETY: This function expects to be called only from within the `TRANSMISSION_EMPTY_IDLE` interrupt.
    pub unsafe fn flush_bytes(&self) {
        assert!(!libarch::interrupts::are_enabled());
        assert!(!self.uart.is_locked());
        assert!(self.pending.is_completed());

        // SAFETY: Mutex lock is already checked.
        let mut uart = unsafe { self.uart.try_lock().unwrap_unchecked() };
        // SAFETY: `Once` completion is already checked.
        let pending_bytes = unsafe { self.pending.get_unchecked() };
        while let Some(bytes) = pending_bytes.pop() {
            Self::write_bytes(&mut uart, &bytes);
        }
    }

    #[doc(hidden)]
    fn _write_str(&self, string: &str) {
        assert!(string.is_ascii());

        match self.pending.get() {
            Some(pending_bytes) => {
                let mut bytes = string.bytes();
                while !bytes.is_empty() {
                    pending_bytes.force_push([
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                        bytes.next().unwrap_or(0),
                    ]);
                }
            }

            None => libarch::interrupts::without(|| {
                let mut uart = self.uart.lock();
                Self::write_bytes(&mut uart, string.as_bytes());
            }),
        }
    }
}

impl core::fmt::Write for SerialWriter {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self._write_str(string);

        Ok(())
    }
}
