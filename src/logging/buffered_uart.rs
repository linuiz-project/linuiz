use circular_buffer::CircularBuffer;
use uart::{Data, FifoControl, LineStatus, Uart, UartAddress};

const UART_FIFO_SIZE: usize = 16;
type BufferChunk = [u8; UART_FIFO_SIZE];

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    #[error("the transmit buffer is full")]
    TxBufferFull,
}

pub struct BufferedUart {
    uart: Uart<Data>,
    tx_buffer: CircularBuffer<64, BufferChunk>,
}

// Safety: Interior address is not thread-specific.
unsafe impl Send for BufferedUart {}

impl BufferedUart {
    /// ## Safety
    ///
    /// - `address` must be a valid serial address pointing to a UART 16550 device.
    /// - `address` must not be read from or written to by another context.
    pub unsafe fn new(address: UartAddress) -> Option<Self> {
        crate::interrupts::without(|| {
            use uart::{Baud, InterruptEnable, LineControl, ModemControl};

            // Safety: Function invariants provide safety guarantees.
            let mut uart = unsafe { Uart::<Data>::new(address) };

            // Bring UART to a known state.
            uart.write_line_control(LineControl::empty());
            uart.write_interrupt_enable(InterruptEnable::empty());

            // Configure the baud rate (tx/rx speed).
            let mut uart = uart.into_configure_mode();
            uart.set_baud(Baud::B115200);
            let mut uart = uart.into_data_mode();

            // Set character size to 8 bits with no parity.
            uart.write_line_control(LineControl::BITS_8);

            // Test the UART to ensure it's functioning correctly.
            uart.write_modem_control(
                ModemControl::REQUEST_TO_SEND | ModemControl::OUT_1 | ModemControl::OUT_2 | ModemControl::LOOPBACK_MODE,
            );

            uart.write_byte(0x1F);
            if uart.read_byte() != 0x1F {
                return None;
            }

            uart.write_fifo_control(FifoControl::ENABLE | FifoControl::CLEAR_RX | FifoControl::CLEAR_TX);
            uart.write_interrupt_enable(InterruptEnable::TRANSMIT_EMPTY);

            // Configure modem control for actual UART usage.
            uart.write_modem_control(ModemControl::TERMINAL_READY | ModemControl::OUT_1 | ModemControl::OUT_2);

            Some(Self { uart, tx_buffer: CircularBuffer::new() })
        })
    }

    #[cfg(debug_assertions)]
    pub fn write_immediate(&mut self, data: &[u8]) {
        for byte in data.iter().copied() {
            while !self.uart.read_line_status().contains(LineStatus::THR_EMPTY) {
                core::hint::spin_loop();
            }

            self.uart.write_byte(byte);
        }
    }

    pub const fn remaining_buffer_size(&self) -> usize {
        (self.tx_buffer.capacity() - self.tx_buffer.len()) * size_of::<BufferChunk>()
    }

    pub fn buffer_data(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.remaining_buffer_size() < data.len() {
            return Err(Error::TxBufferFull);
        }

        let was_empty = self.tx_buffer.is_empty();

        let mut copy_chunk = BufferChunk::default();
        for chunk in data.chunks(size_of::<BufferChunk>()) {
            copy_chunk[..chunk.len()].copy_from_slice(chunk);
            self.tx_buffer.push_back(copy_chunk);
        }

        if was_empty {
            self.write_next();
        }

        Ok(())
    }

    fn unbuffer_chunk(&mut self) -> Option<[u8; UART_FIFO_SIZE]> {
        self.tx_buffer.pop_front()
    }

    fn write_next(&mut self) {
        if let Some(chunk) = self.unbuffer_chunk() {
            for byte in chunk.iter().copied().filter(|byte| *byte == 0) {
                self.uart.write_byte(byte);
            }
        }
    }
}
