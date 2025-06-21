/// A debug output utilizing QEMU's port 0xE9 hack.
pub struct DebugWriter;

impl core::fmt::Write for DebugWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(write_char);

        Ok(())
    }

    fn write_char(&mut self, c: char) -> core::fmt::Result {
        write_char(c);

        Ok(())
    }
}

fn write_char(c: char) {
    // Safety: It's assumed that this port exists if the kernel was compiled and run in debug mode.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!(
            "out 0xe9, al",
            in("al") u8::try_from(c).unwrap_or(b'?'),
            options(nostack, nomem, preserves_flags)
        );
    }
}
