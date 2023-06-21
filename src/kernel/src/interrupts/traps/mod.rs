mod syscall;

use crate::{
    interrupts::Vector,
    task::{Registers, State},
};

/// ### Safety
///
/// This function should only be called in the case of passing context to handle an interrupt.
/// Calling this function more than once and/or outside the context of an interrupt is undefined behaviour.
#[doc(hidden)]
#[inline(never)]
pub unsafe fn handle_trap(irq_vector: u64, state: &mut State, regs: &mut Registers) {
    match Vector::try_from(irq_vector) {
        Ok(Vector::Timer) => crate::cpu::state::with_scheduler(|scheduler| scheduler.interrupt_task(state, regs)),

        Ok(Vector::Syscall) => handle_syscall(state, regs),

        Err(err) => panic!("Invalid interrupt vector: {:X?}", err),
        vector_result => unimplemented!("Unhandled interrupt: {:?}", vector_result),
    }

    crate::cpu::state::end_of_interrupt().unwrap();
}

#[allow(clippy::similar_names)]
fn handle_syscall(state: &mut State, regs: &mut Registers) {
    let vector = regs.rax;
    let arg0 = regs.rdi;
    let arg1 = regs.rsi;
    let arg2 = regs.rdx;
    let arg3 = regs.rcx;
    let arg4 = regs.r8;
    let arg5 = regs.r9;

    let result = syscall::process(vector, arg0, arg1, arg2, arg3, arg4, arg5, state, regs);
    let (rdi, rsi) = <libsys::syscall::Result as libsys::syscall::ResultConverter>::into_registers(result);
    regs.rdi = rdi;
    regs.rsi = rsi;
}
