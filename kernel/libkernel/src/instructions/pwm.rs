pub unsafe fn qemu_shutdown() -> ! {
    warn!("Executing a QEMU serial port shutdown.");
    crate::io::port::WriteOnlyPort::<u16>::new(0x604).write(0x2000);

    panic!("shutdown failed for unknown reason")
}