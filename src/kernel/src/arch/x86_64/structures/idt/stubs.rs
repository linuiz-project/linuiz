use crate::{
    arch::x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode, SelectorErrorCode},
    interrupts::{
        Vector,
        exceptions::{ArchException, handle},
    },
    task::Registers,
};

macro_rules! push_gprs {
    () => {
        "
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rsi
        push rdi
        push rdx
        push rcx
        push rbx
        push rax
        "
    };
}

macro_rules! pop_gprs {
    () => {
        "
        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rdi
        pop rsi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15
        "
    };
}

macro_rules! push_ret_frame {
    ($ip_off:literal) => {
        concat!(
            "
            # Copy code segment to `rax`.
            mov rax, [rsp + (",
            stringify!($ip_off + 1),
            " * 8)]

            # We don't want to try and trace a fault in the kernel back to
            # userspace, so we check if we're coming from the kernel.
            cmp rax, 0x8 # are we coming from kernel code?
            je 2f        # if so, don't zero the frame pointer
            xor rbp, rbp # if not, zero the frame pointer

            2:

            # Copy instruction pointer to `rax`.
            mov rax, [rsp + (",
            stringify!($ip_off),
            " * 8)]

            # Push the return frame.
            push rax # push instruction pointer
            push rbp # push frame pointer
            mov rbp, rsp
            "
        )
    };
}

macro_rules! exception_handler {
    ($exception_name:ident, $return_type:ty) => {
        paste::paste! {
            #[unsafe(naked)]
            pub extern "x86-interrupt" fn [<$exception_name _stub>](stack_frame: InterruptStackFrame) -> $return_type {
                // Safety: When has perfect assembly ever caused undefined behaviour?
                unsafe {
                    core::arch::naked_asm!(
                        "cld",

                        push_gprs!(),
                        push_ret_frame!(15),

                        // Move stack frame into first parameter.
                        "lea rdi, [rsp + (17 * 8)]",
                        // Move cached gprs pointer into second parameter.
                        "lea rsi, [rsp + (2 * 8)]",

                        "call {}",

                        // "pop" stack frame.
                        "add rsp, 0x10",
                        pop_gprs!(),

                        "iretq",
                        sym [<$exception_name _handler>]
                    )
                }
            }
        }
    };
}

macro_rules! exception_handler_with_error {
    ($exception_name:ident, $error_ty:ty, $return_type:ty) => {
        paste::paste! {
            #[unsafe(naked)]
            pub extern "x86-interrupt" fn [<$exception_name _stub>](
                stack_frame: InterruptStackFrame,
                error_code: $error_ty
            ) -> $return_type {
                // Safety: When has perfect assembly ever caused undefined behaviour?
                unsafe {
                    core::arch::naked_asm!(
                        "cld",

                        push_gprs!(),
                        push_ret_frame!(16),

                        // Move stack frame into first parameter.
                        "lea rdi, [rsp + (18 * 8)]",
                        // Move error code into second parameter.
                        "mov rsi, [rsp + (17 * 8)]",
                        // Move cached gprs pointer into third parameter.
                        "lea rdx, [rsp + (2 * 8)]",

                        // Align stack for SysV calling convention.
                        "sub rsp, 0x8",

                        "call {}",

                        "add rsp, 0x18", // "pop" the SysV fn-align & stack frame.
                        pop_gprs!(),
                        "add rsp, 0x8",  // "pop" the error code

                        "iretq",
                        sym [<$exception_name _handler>]
                    )
                }
            }
        }
    };
}

macro_rules! irq_stub {
    ($irq_vector:literal) => {
        paste::paste! {
            #[unsafe(naked)]
            pub extern "x86-interrupt" fn [<irq_ $irq_vector>](_: crate::arch::x86_64::structures::idt::InterruptStackFrame) {
                // Safety: This is literally perfect assembly. It's safe because it's perfect.
                unsafe {
                    core::arch::naked_asm!(
                        "cld",

                        push_gprs!(),
                        push_ret_frame!(15),

                        // Move IRQ vector into first parameter.
                        "mov rdi, {}",
                        // Move stack frame into second parameter.
                        "lea rsi, [rsp + (17 * 8)]",
                        // Move cached gprs pointer into third parameter.
                        "lea rdx, [rsp + (2 * 8)]",

                        "call {}",


                        "add rsp, 0x10", // "pop" stack frame
                        pop_gprs!(),

                        "iretq",
                        const $irq_vector,
                        sym irq_handler
                    );
                }
            }
        }
    };
}

/// ## Safety
///
/// This function should not be called directly.
#[allow(clippy::similar_names)]
unsafe extern "sysv64" fn irq_handler(irq_number: u64, isf: &mut InterruptStackFrame, regs: &mut Registers) {
    match Vector::try_from(irq_number) {
        Ok(Vector::Timer) => crate::cpu::state::with_scheduler(|scheduler| scheduler.interrupt_task(isf, regs)),

        Ok(Vector::Syscall) => {
            let vector = regs.rax;
            let arg0 = regs.rdi;
            let arg1 = regs.rsi;
            let arg2 = regs.rdx;
            let arg3 = regs.rcx;
            let arg4 = regs.r8;
            let arg5 = regs.r9;

            let result = crate::interrupts::syscall::process(vector, arg0, arg1, arg2, arg3, arg4, arg5, isf, regs);
            let (rdi, rsi) = <libsys::syscall::Result as libsys::syscall::ResultConverter>::into_registers(result);
            regs.rdi = rdi;
            regs.rsi = rsi;
        }

        Err(err) => panic!("Invalid interrupt vector: {:X?}", err),
        vector_result => unimplemented!("Unhandled interrupt: {:?}", vector_result),
    }

    // Safety:
    unsafe { crate::cpu::state::end_of_interrupt() }.unwrap();
}

exception_handler!(de, ());
extern "sysv64" fn de_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::DivideError(stack_frame, gprs));
}

exception_handler!(db, ());
extern "sysv64" fn db_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Debug(stack_frame, gprs));
}

exception_handler!(nmi, ());
extern "sysv64" fn nmi_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::NonMaskable(stack_frame, gprs));
}

exception_handler!(bp, ());
extern "sysv64" fn bp_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Breakpoint(stack_frame, gprs));
}

exception_handler!(of, ());
extern "sysv64" fn of_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Overflow(stack_frame, gprs));
}

exception_handler!(br, ());
extern "sysv64" fn br_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::BoundRangeExceeded(stack_frame, gprs));
}

exception_handler!(ud, ());
extern "sysv64" fn ud_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::InvalidOpcode(stack_frame, gprs));
}

exception_handler!(na, ());
extern "sysv64" fn na_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::DeviceNotAvailable(stack_frame, gprs));
}

exception_handler_with_error!(df, u64, !);
extern "sysv64" fn df_handler(stack_frame: &InterruptStackFrame, _: u64, gprs: &Registers) -> ! {
    handle(&ArchException::DoubleFault(stack_frame, gprs));

    unreachable!()
}

exception_handler_with_error!(ts, u64, ());
extern "sysv64" fn ts_handler(stack_frame: &InterruptStackFrame, error_code: u64, gprs: &Registers) {
    handle(&ArchException::InvalidTSS(stack_frame, SelectorErrorCode::new(error_code).unwrap(), gprs));
}

exception_handler_with_error!(np, u64, ());
extern "sysv64" fn np_handler(stack_frame: &InterruptStackFrame, error_code: u64, gprs: &Registers) {
    handle(&ArchException::SegmentNotPresent(stack_frame, SelectorErrorCode::new(error_code).unwrap(), gprs));
}

exception_handler_with_error!(ss, u64, ());
extern "sysv64" fn ss_handler(stack_frame: &InterruptStackFrame, error_code: u64, gprs: &Registers) {
    handle(&ArchException::StackSegmentFault(stack_frame, SelectorErrorCode::new(error_code).unwrap(), gprs));
}

exception_handler_with_error!(gp, u64, ());
extern "sysv64" fn gp_handler(stack_frame: &InterruptStackFrame, error_code: u64, gprs: &Registers) {
    handle(&ArchException::GeneralProtectionFault(stack_frame, SelectorErrorCode::new(error_code).unwrap(), gprs));
}

exception_handler_with_error!(pf, PageFaultErrorCode, ());
extern "sysv64" fn pf_handler(stack_frame: &InterruptStackFrame, err: PageFaultErrorCode, gprs: &Registers) {
    handle(&ArchException::PageFault(stack_frame, gprs, err, crate::arch::x86_64::registers::control::CR2::read()));
}

// --- reserved 15

exception_handler!(mf, ());
extern "sysv64" fn mf_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::x87FloatingPoint(stack_frame, gprs));
}

exception_handler_with_error!(ac, u64, ());
extern "sysv64" fn ac_handler(stack_frame: &InterruptStackFrame, error_code: u64, gprs: &Registers) {
    handle(&ArchException::AlignmentCheck(stack_frame, error_code, gprs));
}

exception_handler!(mc, !);
extern "sysv64" fn mc_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) -> ! {
    handle(&ArchException::MachineCheck(stack_frame, gprs));
    // Wait indefinite in case the above exception handler returns control flow.
    crate::interrupts::wait_indefinite()
}

exception_handler!(xm, ());
extern "sysv64" fn xm_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::SimdFlaotingPoint(stack_frame, gprs));
}

exception_handler!(ve, ());
extern "sysv64" fn ve_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Virtualization(stack_frame, gprs));
}

// --- reserved 22-30

// --- triple fault (can't handle)

irq_stub!(32);
irq_stub!(33);
irq_stub!(34);
irq_stub!(35);
irq_stub!(36);
irq_stub!(37);
irq_stub!(38);
irq_stub!(39);
irq_stub!(40);
irq_stub!(41);
irq_stub!(42);
irq_stub!(43);
irq_stub!(44);
irq_stub!(45);
irq_stub!(46);
irq_stub!(47);
irq_stub!(48);
irq_stub!(49);
irq_stub!(50);
irq_stub!(51);
irq_stub!(52);
irq_stub!(53);
irq_stub!(54);
irq_stub!(55);
irq_stub!(56);
irq_stub!(57);
irq_stub!(58);
irq_stub!(59);
irq_stub!(60);
irq_stub!(61);
irq_stub!(62);
irq_stub!(63);
irq_stub!(64);
irq_stub!(65);
irq_stub!(66);
irq_stub!(67);
irq_stub!(68);
irq_stub!(69);
irq_stub!(70);
irq_stub!(71);
irq_stub!(72);
irq_stub!(73);
irq_stub!(74);
irq_stub!(75);
irq_stub!(76);
irq_stub!(77);
irq_stub!(78);
irq_stub!(79);
irq_stub!(80);
irq_stub!(81);
irq_stub!(82);
irq_stub!(83);
irq_stub!(84);
irq_stub!(85);
irq_stub!(86);
irq_stub!(87);
irq_stub!(88);
irq_stub!(89);
irq_stub!(90);
irq_stub!(91);
irq_stub!(92);
irq_stub!(93);
irq_stub!(94);
irq_stub!(95);
irq_stub!(96);
irq_stub!(97);
irq_stub!(98);
irq_stub!(99);
irq_stub!(100);
irq_stub!(101);
irq_stub!(102);
irq_stub!(103);
irq_stub!(104);
irq_stub!(105);
irq_stub!(106);
irq_stub!(107);
irq_stub!(108);
irq_stub!(109);
irq_stub!(110);
irq_stub!(111);
irq_stub!(112);
irq_stub!(113);
irq_stub!(114);
irq_stub!(115);
irq_stub!(116);
irq_stub!(117);
irq_stub!(118);
irq_stub!(119);
irq_stub!(120);
irq_stub!(121);
irq_stub!(122);
irq_stub!(123);
irq_stub!(124);
irq_stub!(125);
irq_stub!(126);
irq_stub!(127);
irq_stub!(128);
irq_stub!(129);
irq_stub!(130);
irq_stub!(131);
irq_stub!(132);
irq_stub!(133);
irq_stub!(134);
irq_stub!(135);
irq_stub!(136);
irq_stub!(137);
irq_stub!(138);
irq_stub!(139);
irq_stub!(140);
irq_stub!(141);
irq_stub!(142);
irq_stub!(143);
irq_stub!(144);
irq_stub!(145);
irq_stub!(146);
irq_stub!(147);
irq_stub!(148);
irq_stub!(149);
irq_stub!(150);
irq_stub!(151);
irq_stub!(152);
irq_stub!(153);
irq_stub!(154);
irq_stub!(155);
irq_stub!(156);
irq_stub!(157);
irq_stub!(158);
irq_stub!(159);
irq_stub!(160);
irq_stub!(161);
irq_stub!(162);
irq_stub!(163);
irq_stub!(164);
irq_stub!(165);
irq_stub!(166);
irq_stub!(167);
irq_stub!(168);
irq_stub!(169);
irq_stub!(170);
irq_stub!(171);
irq_stub!(172);
irq_stub!(173);
irq_stub!(174);
irq_stub!(175);
irq_stub!(176);
irq_stub!(177);
irq_stub!(178);
irq_stub!(179);
irq_stub!(180);
irq_stub!(181);
irq_stub!(182);
irq_stub!(183);
irq_stub!(184);
irq_stub!(185);
irq_stub!(186);
irq_stub!(187);
irq_stub!(188);
irq_stub!(189);
irq_stub!(190);
irq_stub!(191);
irq_stub!(192);
irq_stub!(193);
irq_stub!(194);
irq_stub!(195);
irq_stub!(196);
irq_stub!(197);
irq_stub!(198);
irq_stub!(199);
irq_stub!(200);
irq_stub!(201);
irq_stub!(202);
irq_stub!(203);
irq_stub!(204);
irq_stub!(205);
irq_stub!(206);
irq_stub!(207);
irq_stub!(208);
irq_stub!(209);
irq_stub!(210);
irq_stub!(211);
irq_stub!(212);
irq_stub!(213);
irq_stub!(214);
irq_stub!(215);
irq_stub!(216);
irq_stub!(217);
irq_stub!(218);
irq_stub!(219);
irq_stub!(220);
irq_stub!(221);
irq_stub!(222);
irq_stub!(223);
irq_stub!(224);
irq_stub!(225);
irq_stub!(226);
irq_stub!(227);
irq_stub!(228);
irq_stub!(229);
irq_stub!(230);
irq_stub!(231);
irq_stub!(232);
irq_stub!(233);
irq_stub!(234);
irq_stub!(235);
irq_stub!(236);
irq_stub!(237);
irq_stub!(238);
irq_stub!(239);
irq_stub!(240);
irq_stub!(241);
irq_stub!(242);
irq_stub!(243);
irq_stub!(244);
irq_stub!(245);
irq_stub!(246);
irq_stub!(247);
irq_stub!(248);
irq_stub!(249);
irq_stub!(250);
irq_stub!(251);
irq_stub!(252);
irq_stub!(253);
irq_stub!(254);
irq_stub!(255);
