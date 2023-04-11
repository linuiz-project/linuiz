#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rfl: u64,
    pub rsp: u64,
}

/// ### Safety
///
/// This function should never be called by software.
#[allow(clippy::similar_names, clippy::no_effect_underscore_binding)]
pub unsafe extern "sysv64" fn sanitize(
    vector: u64,
    arg0: u64,
    arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    ret_ip: u64,
    ret_sp: u64,
    mut _regs: Registers,
) -> crate::proc::State {
    let syscall = match vector {
        0x100 => {
            use log::Level;

            let log_level = match arg0 {
                1 => Ok(Level::Error),
                2 => Ok(Level::Warn),
                3 => Ok(Level::Info),
                4 => Ok(Level::Debug),
                arg0 => Err(arg0),
            };

            match log_level {
                Ok(level) => Some(Syscall::Log { level, cstr_ptr: usize::try_from(arg1).unwrap() as *const _ }),
                Err(invalid_level) => {
                    warn!("Invalid log level provided: {}", invalid_level);
                    None
                }
            }
        }

        vector => {
            warn!("Unhandled system call vector: {:#X}", vector);
            None
        }
    };

    if let Some(syscall) = syscall {
        process(syscall);
    } else {
        warn!("Failed to execute system call.");
    }

    crate::proc::State { ip: ret_ip, sp: ret_sp }
}

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
                unsafe {
                    crate::memory::catch_read_str(core::ptr::NonNull::new(cstr_ptr.cast_mut().cast()).unwrap()).unwrap()
                }
            );
        }
    }
}
