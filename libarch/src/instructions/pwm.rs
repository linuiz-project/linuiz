/// Executes a simple QEMU port-based shutdown.
pub unsafe fn qemu_shutdown() -> ! {
    crate::io::port::write16(0x604, 0x2000);

    panic!("shutdown failed for unknown reason")
}
