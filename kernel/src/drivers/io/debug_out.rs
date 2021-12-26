#![allow(dead_code)]

use libstd::io::port::WriteOnlyPort;

const QEMU_PORT_E9: u16 = 0xE9;

pub struct DebugOut {
    out: WriteOnlyPort<u8>,
}

impl DebugOut {
    pub const fn new() -> Self {
        Self {
            out: unsafe { WriteOnlyPort::new(QEMU_PORT_E9) },
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.out.write(byte)
    }

    pub fn write_bytes(&mut self, bytes: core::str::Bytes) {
        for byte in bytes {
            self.write_byte(byte)
        }
    }

    pub fn write_str(&mut self, string: &str) {
        for byte in string.bytes() {
            self.out.write(byte);
        }
    }
}

impl core::fmt::Write for DebugOut {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_str(string);
        Ok(())
    }
}
