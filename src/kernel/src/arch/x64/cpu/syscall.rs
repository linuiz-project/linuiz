use crate::syscall::Syscall;

/// SAFETY: This function should never be called by software—it is the entrypoint for the x86_64 `syscall` instruction.
#[naked]
pub unsafe extern "sysv64" fn syscall_handler() {
    core::arch::asm!(
        "
        cld
        mov rax, rsp                # save the userspace rsp

        swapgs                      # `swapgs` to switch to kernel stack
        mov rsp, gs:0x0             # switch to kernel stack
        swapgs                      # `swapgs` to allow software to use `IA32_KERNEL_GS_BASE` again

        # preserve registers according to SysV ABI spec
        push rax    # this pushes the userspace `rsp`
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15

        # push the preserved registers pointer as stack argument
        push rsp
        # push return context as stack arguments
        push rax    
        push rcx
        # caller already passed their own arguments in relevant registers
        call {}

        # taregt `rsp` stored in `rax`
        mov [rsp + (6 * 8)], rax    # updates userspace `rsp` on stack
        # rip stored in rdx
        mov rcx, rax                # updates sysret `rip`

        # restore preserved registers
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        pop rsp     # this restores userspace `rsp`

        sysret
        ",
        sym syscall_handler_inner,
        options(noreturn)
    )
}

#[repr(C)]
#[derive(Debug)]
struct PreservedRegisters {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    rsp: u64,
}

#[repr(C, packed)]
struct ReturnContext {
    ip: u64,
    sp: u64,
}

/// Handler for executing system calls from userspace.
extern "sysv64" fn syscall_handler_inner(
    vector: u64,
    rsi: u64,
    rdx: u64,
    _rcx: u64,
    _r8: u64,
    _r9: u64,
    ret_ip: u64,
    ret_sp: u64,
    _regs: &mut PreservedRegisters,
) -> ReturnContext {
    let syscall = match vector {
        0x100 => {
            use log::Level;

            // TODO possibly PR the `log` crate to make `log::Level::from_usize()` public.
            let log_level = match rsi {
                1 => Ok(Level::Error),
                2 => Ok(Level::Warn),
                3 => Ok(Level::Info),
                4 => Ok(Level::Debug),
                5 => Ok(Level::Trace),
                rsi => Err(rsi),
            };

            match log_level {
                Ok(level) => Some(Syscall::Log { level, cstr_ptr: rdx as usize as *const _ }),
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

    match syscall {
        Some(syscall) => crate::syscall::do_syscall(syscall),
        None => warn!("Failed to execute system call due to errors."),
    }

    ReturnContext { ip: ret_ip, sp: ret_sp }
}
