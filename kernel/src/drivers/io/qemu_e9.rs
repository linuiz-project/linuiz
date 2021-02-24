#![allow(dead_code)]

use libkernel::io::port::WriteOnlyPort;

const QEMU_PORT_E9: u16 = 0xE9;

pub struct QEMUE9 {
    out: WriteOnlyPort<u8>,
}

impl QEMUE9 {
    pub const fn new() -> Self {
        Self {
            out: unsafe { WriteOnlyPort::new(QEMU_PORT_E9) },
        }
    }

    pub fn write_str(&mut self, string: &str) {
        for byte in string.bytes() {
            self.out.write(byte);
        }
    }
}

impl core::fmt::Write for QEMUE9 {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_str(string);
        Ok(())
    }
}
