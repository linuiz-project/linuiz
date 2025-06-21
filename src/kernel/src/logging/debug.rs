// Safety: It's assumed that this port exists if the kernel was compiled in debug mode.
static mut QEMU_E9: ioports::WriteOnlyPort<u32> = unsafe { ioports::WriteOnlyPort::new(0xE9) };

struct DebugWriter;

impl core::fmt::Write for DebugWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c| {
            // Safety: The QEMU 0xE9 port is very fast and does not need to be mutexed.
            //         The caller is required to ensure that the output from this makes sense.
            unsafe {
                QEMU_E9.write(u32::from(c));
            }
        });

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        // Safety: The QEMU 0xE9 port is very fast and does not need to be mutexed.
        //         The caller is required to ensure that the output from this makes sense.
        unsafe {
            QEMU_E9.write(u32::from(c));
        }

        Ok(())
    }
}
