use libkernel::io::port::WriteOnlyPort;

pub struct EmulatorOut {
    out: WriteOnlyPort<u8>,
}

impl EmulatorOut {
    pub fn write_str(&mut self, string: &str) {
        for byte in string.bytes() {
            self.out.write(byte);
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.out.write(*byte);
        }
    }
}

impl core::fmt::Write for EmulatorOut {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.write_str(string);
        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref EMULATOR_OUT: spin::Mutex<EmulatorOut> = spin::Mutex::new(EmulatorOut {
        out: unsafe { WriteOnlyPort::new(0xE9) }
    });
}
