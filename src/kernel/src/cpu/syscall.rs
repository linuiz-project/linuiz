use core::ptr::NonNull;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syscall {
    /// Logs to the kernel standard output.
    ///
    /// Vector: 0x100
    Log { level: log::Level, cstr_ptr: *const core::ffi::c_char },
}

pub fn process(vector: Syscall) {
    match vector {
        Syscall::Log { level, cstr_ptr } => {
            log!(
                level,
                "Syscall: Log: {:?}",
                // Safety: If pointer is null, we panic.
                // TODO do not panic.
                unsafe { crate::memory::catch_read_str(NonNull::new(cstr_ptr.cast_mut().cast()).unwrap()).unwrap() }
            );
        }
    }
}
