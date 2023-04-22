use libsys::syscall::{Result, Vector};

/// ### Safety
///
/// This function should never be called by software.
#[naked]
#[doc(hidden)]
pub(super) unsafe extern "sysv64" fn _syscall_entry() {
    core::arch::asm!(
        "
        cld

        mov rax, rsp                # save the userspace rsp
        swapgs                      # `swapgs` to switch to kernel stack
        mov rsp, gs:0x0             # switch to kernel stack
        swapgs                      # `swapgs` to allow software to use `IA32_KERNEL_GS_BASE` again

        push rax    # push userspace `rsp`
        push r11    # push usersapce `rflags`
        push rcx    # push userspace `rip`

        # preserve registers according to SysV ABI spec
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15

        # `r13`, `r14`, `r15` are scratch
        lea r13, [rsp + 0x0]        # load registers ptr
        lea r14, [rsp + (8 * 0x8)]  # load sp ptr
        lea r15, [rsp + (6 * 0x8)]  # load ip ptr

        # push stack arguments
        push r15
        push r14
        push r13

        # caller passed arguments
        call {}
        # return values in rax:rdx

        add rsp, 0x18

        # restore preserved registers
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx

        pop rcx     # restore userspace `rip`
        pop r11     # restore userspace `rflags`
        pop rsp     # restore userspace `rsp`

        sysretq
        ",
        sym crate::cpu::syscall::sanitize,
        options(noreturn)
    )
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallScheme {
    UnhandledVector(u64),

    Klog { level: log::Level, str_ptr: *const u8, str_len: usize },
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
    _ret_ip: &mut u64,
    _ret_sp: &mut u64,
    _regs: &mut Registers,
) -> Result {
    let syscall = match Vector::try_from(vector) {
        Ok(Vector::KlogInfo) => SyscallScheme::Klog {
            level: log::Level::Info,
            str_ptr: usize::try_from(arg0).map_err(Result::from)? as *mut u8,
            str_len: usize::try_from(arg1).map_err(Result::from)?,
        },

        Ok(Vector::KlogError) => SyscallScheme::Klog {
            level: log::Level::Error,
            str_ptr: usize::try_from(arg0).map_err(Result::from)? as *mut u8,
            str_len: usize::try_from(arg1).map_err(Result::from)?,
        },

        Ok(Vector::KlogDebug) => SyscallScheme::Klog {
            level: log::Level::Debug,
            str_ptr: usize::try_from(arg0).map_err(Result::from)? as *mut u8,
            str_len: usize::try_from(arg1).map_err(Result::from)?,
        },

        Ok(Vector::KlogTrace) => SyscallScheme::Klog {
            level: log::Level::Trace,
            str_ptr: usize::try_from(arg0).map_err(Result::from)? as *mut u8,
            str_len: usize::try_from(arg1).map_err(Result::from)?,
        },

        Err(err) => SyscallScheme::UnhandledVector(err.number),
    };

    process(syscall)
}

pub fn process(scheme: SyscallScheme) -> Result {
    match scheme {
        SyscallScheme::UnhandledVector(vector) => {
            warn!("Unhandled system call vector: {:#X}", vector);

            Result::InvalidVector
        }

        SyscallScheme::Klog { level, str_ptr, str_len } => {
            let str =
                core::str::from_utf8(unsafe { core::slice::from_raw_parts(str_ptr, str_len) }).map_err(Result::from)?;
            log!(level, "[KLOG]: {}", str);

            Result::Ok
        }
    }
}
