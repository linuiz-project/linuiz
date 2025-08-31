use super::with_buffered_uart;
use crate::logging::with_formatted_log_record;
use core::num::NonZero;
use uart::{LineStatus, address::PortAddress};



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

        super::configure_uart(address, false);

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
                super::with_buffered_uart(|string_buffer, uart| {
                    core::fmt::Write::write_fmt(string_buffer, args).ok();

                     string_buffer
                        .chars().enumerate().for_each(|(iteration, char)| {

                            if (iteration & UART) == 0 {
 while !uart.read_line_status().contains(LineStatus::THR_EMPTY) {
                                core::hint::spin_loop();
                            }
}
});

                        .chunks(UART_FIFO_SIZE)
                        .for_each(|chunk| {
                           

                            chunk.iter().copied().for_each(|byte| uart.write_byte(byte));
                        });

                    string_buffer.clear();
                });
            });
        }
    }

    fn flush(&self) {
        unimplemented!()
    }
}
