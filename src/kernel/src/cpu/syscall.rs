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
    ret_ip: &mut u64,
    ret_sp: &mut u64,
    regs: &mut Registers,
) -> Result<()> {
    let syscall = match vector {
        libsys::syscall::Vector::SyslogInfo => SyscallScheme::Klog {
            level: log::Level::Info,
            str_ptr: usize::try_from(arg0).unwrap() as *mut u8,
            str_len: usize::try_from(arg1).unwrap(),
        },

        libsys::syscall::Vector::SyslogError => SyscallScheme::Klog {
            level: log::Level::Error,
            str_ptr: usize::try_from(arg0).unwrap() as *mut u8,
            str_len: usize::try_from(arg1).unwrap(),
        },

        libsys::syscall::Vector::SyslogDebug => SyscallScheme::Klog {
            level: log::Level::Debug,
            str_ptr: usize::try_from(arg0).unwrap() as *mut u8,
            str_len: usize::try_from(arg1).unwrap(),
        },

        libsys::syscall::Vector::SyslogTrace => SyscallScheme::Klog {
            level: log::Level::Trace,
            str_ptr: usize::try_from(arg0).unwrap() as *mut u8,
            str_len: usize::try_from(arg1).unwrap(),
        },

        vector => SyscallScheme::UnhandledVector,
    };

    process(syscall);

    crate::proc::State::user(ret_ip, ret_sp)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallScheme {
    UnhandledVector(u64),

    Klog { level: log::Level, str_ptr: *const u8, str_len: usize },
}

pub fn process(vector: Syscall) -> libsys::syscall::Result<()> {
    match vector {
        SyscallScheme::UnhandledVector(vector) => {
            warn!("Unhandled system call vector: {:#X}", vector);
        }

        SyscallScheme::Klog { level, str_ptr, str_len } => {
            log!(
                level,
                "[LOG]: {}",
                // Safety: If pointer is null, we panic.
                // TODO do not panic.
                unsafe {
                    crate::memory::catch_read_str(core::ptr::NonNull::new(cstr_ptr.cast_mut().cast()).unwrap()).unwrap()
                }
            );
        }
    }
}
