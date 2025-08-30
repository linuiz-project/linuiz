use crate::{
    logging::{debug::DebugLogger, with_formatted_log_record},
    util::sync::Mutex,
};
use core::num::NonZero;
use uart::address::PortAddress;

const UART_FIFO_SIZE: usize = 16;
type Chunk = [u8; UART_FIFO_SIZE];
type Chunks = heapless::mpmc::Queue<Chunk, 1024>;

static CHUNKS: Chunks = Chunks::new();

struct WriteBuffer<const SIZE: usize> {
    buffer: [u8; SIZE],
    len: usize,
}

impl<const SIZE: usize> WriteBuffer<SIZE> {
    fn remaining_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[self.len..]
    }

    fn current(&self) -> &[u8] {
        &self.buffer[..self.len]
    }

    fn reset(&mut self) {
        self.len = 0;
    }
}

impl<const SIZE: usize> core::fmt::Write for WriteBuffer<SIZE> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.remaining_mut();

        if bytes.len() < remaining.len() {
            remaining[..bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        } else {
            remaining.copy_from_slice(&bytes[..remaining.len()]);

            if let Some(last_byte) = remaining.last_mut() {
                *last_byte = b'\n';
            }

            self.len += remaining.len();
        }

        Ok(())
    }
}

static WRITE_BUFFER: Mutex<WriteBuffer<0x2000>> = Mutex::new(WriteBuffer {
    buffer: [0u8; _],
    len: 0,
});

pub struct Logger;

impl Logger {
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

        super::configure_uart(address, true);

        &Self
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        cfg_select! {
                debug_assertions  => { metadata.level() <= log::Level::Trace }
            not(debug_assertions) => { metadata.level() <= log::Level::Debug }
        }
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            with_formatted_log_record(record, |args| {
                WRITE_BUFFER.with_lock(|write_buffer| {
                    core::fmt::Write::write_fmt(write_buffer, args).unwrap();

                    writeln!(
                        &mut DebugLogger as &mut dyn core::fmt::Write,
                        "BUFFERED: {} BYTES",
                        write_buffer.len
                    );

                    let mut chunks = write_buffer
                        .current()
                        .iter()
                        .copied()
                        .array_chunks::<UART_FIFO_SIZE>();

                    let enqueue = |chunk: Chunk| {
                        while CHUNKS.enqueue(chunk).is_err() {
                            // ... and spin until there's space for it.
                            core::hint::spin_loop();
                        }
                    };

                    let len = chunks.len();
                    writeln!(
                        &mut DebugLogger as &mut dyn core::fmt::Write,
                        "ENQUEUEING: {len} CHUNKS"
                    );

                    // Iterate & enqueue each chunk ...
                    chunks.by_ref().enumerate().for_each(|(iteration, bytes)| {
                        writeln!(
                            &mut DebugLogger as &mut dyn core::fmt::Write,
                            "ENQUEUEING: {}/{len} locked",
                            iteration + 1
                        );

                        enqueue(bytes);
                    });

                    writeln!(
                        &mut DebugLogger as &mut dyn core::fmt::Write,
                        "ENQUEUED CHUNKS"
                    );

                    // Enqueue the remaining bytes ...
                    if let Some(remainder) = chunks.into_remainder() {
                        let mut remainder_chunk = Chunk::default();
                        remainder.enumerate().for_each(|(index, byte)| {
                            remainder_chunk[index] = byte;
                        });

                        enqueue(remainder_chunk);
                    }
                    writeln!(
                        &mut DebugLogger as &mut dyn core::fmt::Write,
                        "ENQUEUED REMAINING"
                    );

                    write_buffer.reset();
                });
            });
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}

pub unsafe fn send_next_chunk() {
    super::with_uart(|uart| {
        #[cfg(debug_assertions)]
        {
            let line_status = uart.read_line_status();
            assert!(line_status.contains(uart::LineStatus::THR_SHR_EMPTY));
        }

        if let Some(bytes) = CHUNKS.dequeue() {
            bytes
                .into_iter()
                .take_while(|byte| *byte > 0)
                .for_each(|byte| {
                    uart.write_byte(byte);
                });
        } else {
            // TODO
            // Writing a nul-byte to constantly re-trigger the interrupt routine is
            // probably very unperformant. Instead, the transmit should conditionally be
            // started immediately when the queue is empty.

            // Write a nul-byte to re-trigger the interrupt later.
            uart.write_byte(b'\0');
        }
    });
}
