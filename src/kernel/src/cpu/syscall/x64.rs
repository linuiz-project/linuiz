use libsys::syscall::Result;

use crate::{arch::x64::registers::RFlags, proc::State};

/// ### Safety
///
/// This function should never be called by software.
#[naked]
#[doc(hidden)]
pub(in crate::cpu) unsafe extern "sysv64" fn _syscall_entry() {
    core::arch::asm!(
        "
        cld

        mov rax, rsp                # save the userspace rsp
        swapgs                      # `swapgs` to switch to kernel stack
        mov rsp, gs:0x0             # switch to kernel stack
        swapgs                      # `swapgs` to allow software to use `IA32_KERNEL_GS_BASE` again

        push rax        # push userspace `rsp`
        push rcx        # push userspace `rip`
        push r11        # push usersapce `rflags`
        mov rcx, r10    # 4th argument into `rcx`

        # preserve registers according to SysV ABI spec
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15

        # `r13`, `r14`, `r15` are scratch
        lea r12, [rsp + 0x0]        # load registers ptr
        lea r13, [rsp + (8 * 0x8)]  # load sp ptr
        lea r14, [rsp + (7 * 0x8)]  # load ip ptr
        lea r15, [rsp + (6 + 0x8)]  # load rflags

        # push stack arguments
        push r15
        push r14
        push r13
        push r12

        # caller passed arguments
        call {}
        # return values in rax:rdx

        # clean up stack arguments
        add rsp, 0x20

        # restore preserved registers
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx

        pop r11     # restore userspace `rflags`
        pop rcx     # restore userspace `rip`
        pop rsp     # restore userspace `rsp`

        sysretq
        ",
        sym translate,
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
}

unsafe extern "sysv64" fn translate(
    vector: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    regs: &mut Registers,
    sp: &mut u64,
    ip: &mut u64,
    rfl: &mut RFlags,
) -> Result {
    let mut tmp_state = State::user(*ip, *sp);
    let mut tmp_regs = crate::proc::Registers::default();

    tmp_state.rfl = *rfl;
    tmp_regs.rbx = regs.rbx;
    tmp_regs.rbp = regs.rbp;
    tmp_regs.r12 = regs.r12;
    tmp_regs.r13 = regs.r13;
    tmp_regs.r14 = regs.r14;
    tmp_regs.r15 = regs.r15;

    let result = super::process(vector, arg0, arg1, arg2, arg3, arg4, &mut tmp_state, &mut tmp_regs);

    *ip = tmp_state.ip;
    *sp = tmp_state.sp;
    *rfl = tmp_state.rfl;
    regs.rbx = tmp_regs.rbx;
    regs.rbp = tmp_regs.rbp;
    regs.r12 = tmp_regs.r12;
    regs.r13 = tmp_regs.r13;
    regs.r14 = tmp_regs.r14;
    regs.r15 = tmp_regs.r15;

    result
}
