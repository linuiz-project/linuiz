use crate::{
    LinkerSymbol,
    arch::x86_64::{
        devices::x2apic::x2Apic,
        structures::idt::{InterruptStackFrame, PageFaultErrorCode, SelectorErrorCode},
    },
    cpu::local_state::LocalState,
    interrupts::{
        Vector,
        exceptions::{ArchException, handle},
    },
    task::Registers,
};

#[unsafe(no_mangle)]
extern "sysv64" fn __de_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::DivideError(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __db_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Debug(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __nm_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::NonMaskable(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __bp_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Breakpoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __of_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Overflow(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __br_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::BoundRangeExceeded(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ud_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::InvalidOpcode(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __na_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::DeviceNotAvailable(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __df_handler(stack_frame: &InterruptStackFrame, _: u64, gprs: &Registers) {
    handle(&ArchException::DoubleFault(stack_frame, gprs));
    unreachable!("#DF cannot be recovered from");
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ts_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(&ArchException::InvalidTSS(
        stack_frame,
        SelectorErrorCode::new(error_code).unwrap(),
        gprs,
    ));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __np_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(&ArchException::SegmentNotPresent(
        stack_frame,
        SelectorErrorCode::new(error_code).unwrap(),
        gprs,
    ));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ss_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(&ArchException::StackSegmentFault(
        stack_frame,
        SelectorErrorCode::new(error_code).unwrap(),
        gprs,
    ));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __gp_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(&ArchException::GeneralProtectionFault(
        stack_frame,
        SelectorErrorCode::new(error_code).unwrap(),
        gprs,
    ));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __pf_handler(
    stack_frame: &InterruptStackFrame,
    err: PageFaultErrorCode,
    gprs: &Registers,
) {
    handle(&ArchException::PageFault(
        stack_frame,
        gprs,
        err,
        crate::arch::x86_64::registers::control::CR2::read(),
    ));
}

// --- reserved 15

#[unsafe(no_mangle)]
extern "sysv64" fn __mf_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::x87FloatingPoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ac_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(&ArchException::AlignmentCheck(
        stack_frame,
        error_code,
        gprs,
    ));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __mc_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::MachineCheck(stack_frame, gprs));
    unreachable!("#MC cannot be recovered");
}

#[unsafe(no_mangle)]
extern "sysv64" fn __xm_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::SimdFlaotingPoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ve_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(&ArchException::Virtualization(stack_frame, gprs));
}

// --- reserved 22-30
// --- triple fault (can't handle)

#[unsafe(no_mangle)]
#[allow(clippy::similar_names)]
extern "sysv64" fn __irq_handler(
    irq_number: u8,
    isf: &mut InterruptStackFrame,
    regs: &mut Registers,
) {
    match Vector::from(irq_number) {
        Vector::Timer => {
            LocalState::with_scheduler(|scheduler| {
                scheduler.interrupt_task(isf, regs);
            });
        }

        Vector::Syscall => {
            let vector = regs.rax;
            let arg0 = regs.rdi;
            let arg1 = regs.rsi;
            let arg2 = regs.rdx;
            let arg3 = regs.rcx;
            let arg4 = regs.r8;
            let arg5 = regs.r9;
            let result = crate::interrupts::syscall::process(
                vector, arg0, arg1, arg2, arg3, arg4, arg5, isf, regs,
            );
            let (rdi, rsi) =
                <libsys::syscall::Result as libsys::syscall::ResultConverter>::into_registers(
                    result,
                );
            regs.rdi = rdi;
            regs.rsi = rsi;
        }
        vector => unimplemented!("unsupported interrupt vector: {vector:?}"),
    }

    // Safety: This is the end of an interrupt context.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        x2Apic::end_of_interrupt();
    }
}

unsafe extern "C" {
    pub unsafe static __de_stub: LinkerSymbol;
    pub unsafe static __db_stub: LinkerSymbol;
    pub unsafe static __nm_stub: LinkerSymbol;
    pub unsafe static __bp_stub: LinkerSymbol;
    pub unsafe static __of_stub: LinkerSymbol;
    pub unsafe static __br_stub: LinkerSymbol;
    pub unsafe static __ud_stub: LinkerSymbol;
    pub unsafe static __na_stub: LinkerSymbol;
    pub unsafe static __df_stub: LinkerSymbol;
    pub unsafe static __ts_stub: LinkerSymbol;
    pub unsafe static __np_stub: LinkerSymbol;
    pub unsafe static __ss_stub: LinkerSymbol;
    pub unsafe static __gp_stub: LinkerSymbol;
    pub unsafe static __pf_stub: LinkerSymbol;
    pub unsafe static __mf_stub: LinkerSymbol;
    pub unsafe static __ac_stub: LinkerSymbol;
    pub unsafe static __mc_stub: LinkerSymbol;
    pub unsafe static __xm_stub: LinkerSymbol;
    pub unsafe static __ve_stub: LinkerSymbol;
    pub unsafe static __irq_32_stub: LinkerSymbol;
    pub unsafe static __irq_33_stub: LinkerSymbol;
    pub unsafe static __irq_34_stub: LinkerSymbol;
    pub unsafe static __irq_35_stub: LinkerSymbol;
    pub unsafe static __irq_36_stub: LinkerSymbol;
    pub unsafe static __irq_37_stub: LinkerSymbol;
    pub unsafe static __irq_38_stub: LinkerSymbol;
    pub unsafe static __irq_39_stub: LinkerSymbol;
    pub unsafe static __irq_40_stub: LinkerSymbol;
    pub unsafe static __irq_41_stub: LinkerSymbol;
    pub unsafe static __irq_42_stub: LinkerSymbol;
    pub unsafe static __irq_43_stub: LinkerSymbol;
    pub unsafe static __irq_44_stub: LinkerSymbol;
    pub unsafe static __irq_45_stub: LinkerSymbol;
    pub unsafe static __irq_46_stub: LinkerSymbol;
    pub unsafe static __irq_47_stub: LinkerSymbol;
    pub unsafe static __irq_48_stub: LinkerSymbol;
    pub unsafe static __irq_49_stub: LinkerSymbol;
    pub unsafe static __irq_50_stub: LinkerSymbol;
    pub unsafe static __irq_51_stub: LinkerSymbol;
    pub unsafe static __irq_52_stub: LinkerSymbol;
    pub unsafe static __irq_53_stub: LinkerSymbol;
    pub unsafe static __irq_54_stub: LinkerSymbol;
    pub unsafe static __irq_55_stub: LinkerSymbol;
    pub unsafe static __irq_56_stub: LinkerSymbol;
    pub unsafe static __irq_57_stub: LinkerSymbol;
    pub unsafe static __irq_58_stub: LinkerSymbol;
    pub unsafe static __irq_59_stub: LinkerSymbol;
    pub unsafe static __irq_60_stub: LinkerSymbol;
    pub unsafe static __irq_61_stub: LinkerSymbol;
    pub unsafe static __irq_62_stub: LinkerSymbol;
    pub unsafe static __irq_63_stub: LinkerSymbol;
    pub unsafe static __irq_64_stub: LinkerSymbol;
    pub unsafe static __irq_65_stub: LinkerSymbol;
    pub unsafe static __irq_66_stub: LinkerSymbol;
    pub unsafe static __irq_67_stub: LinkerSymbol;
    pub unsafe static __irq_68_stub: LinkerSymbol;
    pub unsafe static __irq_69_stub: LinkerSymbol;
    pub unsafe static __irq_70_stub: LinkerSymbol;
    pub unsafe static __irq_71_stub: LinkerSymbol;
    pub unsafe static __irq_72_stub: LinkerSymbol;
    pub unsafe static __irq_73_stub: LinkerSymbol;
    pub unsafe static __irq_74_stub: LinkerSymbol;
    pub unsafe static __irq_75_stub: LinkerSymbol;
    pub unsafe static __irq_76_stub: LinkerSymbol;
    pub unsafe static __irq_77_stub: LinkerSymbol;
    pub unsafe static __irq_78_stub: LinkerSymbol;
    pub unsafe static __irq_79_stub: LinkerSymbol;
    pub unsafe static __irq_80_stub: LinkerSymbol;
    pub unsafe static __irq_81_stub: LinkerSymbol;
    pub unsafe static __irq_82_stub: LinkerSymbol;
    pub unsafe static __irq_83_stub: LinkerSymbol;
    pub unsafe static __irq_84_stub: LinkerSymbol;
    pub unsafe static __irq_85_stub: LinkerSymbol;
    pub unsafe static __irq_86_stub: LinkerSymbol;
    pub unsafe static __irq_87_stub: LinkerSymbol;
    pub unsafe static __irq_88_stub: LinkerSymbol;
    pub unsafe static __irq_89_stub: LinkerSymbol;
    pub unsafe static __irq_90_stub: LinkerSymbol;
    pub unsafe static __irq_91_stub: LinkerSymbol;
    pub unsafe static __irq_92_stub: LinkerSymbol;
    pub unsafe static __irq_93_stub: LinkerSymbol;
    pub unsafe static __irq_94_stub: LinkerSymbol;
    pub unsafe static __irq_95_stub: LinkerSymbol;
    pub unsafe static __irq_96_stub: LinkerSymbol;
    pub unsafe static __irq_97_stub: LinkerSymbol;
    pub unsafe static __irq_98_stub: LinkerSymbol;
    pub unsafe static __irq_99_stub: LinkerSymbol;
    pub unsafe static __irq_100_stub: LinkerSymbol;
    pub unsafe static __irq_101_stub: LinkerSymbol;
    pub unsafe static __irq_102_stub: LinkerSymbol;
    pub unsafe static __irq_103_stub: LinkerSymbol;
    pub unsafe static __irq_104_stub: LinkerSymbol;
    pub unsafe static __irq_105_stub: LinkerSymbol;
    pub unsafe static __irq_106_stub: LinkerSymbol;
    pub unsafe static __irq_107_stub: LinkerSymbol;
    pub unsafe static __irq_108_stub: LinkerSymbol;
    pub unsafe static __irq_109_stub: LinkerSymbol;
    pub unsafe static __irq_110_stub: LinkerSymbol;
    pub unsafe static __irq_111_stub: LinkerSymbol;
    pub unsafe static __irq_112_stub: LinkerSymbol;
    pub unsafe static __irq_113_stub: LinkerSymbol;
    pub unsafe static __irq_114_stub: LinkerSymbol;
    pub unsafe static __irq_115_stub: LinkerSymbol;
    pub unsafe static __irq_116_stub: LinkerSymbol;
    pub unsafe static __irq_117_stub: LinkerSymbol;
    pub unsafe static __irq_118_stub: LinkerSymbol;
    pub unsafe static __irq_119_stub: LinkerSymbol;
    pub unsafe static __irq_120_stub: LinkerSymbol;
    pub unsafe static __irq_121_stub: LinkerSymbol;
    pub unsafe static __irq_122_stub: LinkerSymbol;
    pub unsafe static __irq_123_stub: LinkerSymbol;
    pub unsafe static __irq_124_stub: LinkerSymbol;
    pub unsafe static __irq_125_stub: LinkerSymbol;
    pub unsafe static __irq_126_stub: LinkerSymbol;
    pub unsafe static __irq_127_stub: LinkerSymbol;
    pub unsafe static __irq_128_stub: LinkerSymbol;
    pub unsafe static __irq_129_stub: LinkerSymbol;
    pub unsafe static __irq_130_stub: LinkerSymbol;
    pub unsafe static __irq_131_stub: LinkerSymbol;
    pub unsafe static __irq_132_stub: LinkerSymbol;
    pub unsafe static __irq_133_stub: LinkerSymbol;
    pub unsafe static __irq_134_stub: LinkerSymbol;
    pub unsafe static __irq_135_stub: LinkerSymbol;
    pub unsafe static __irq_136_stub: LinkerSymbol;
    pub unsafe static __irq_137_stub: LinkerSymbol;
    pub unsafe static __irq_138_stub: LinkerSymbol;
    pub unsafe static __irq_139_stub: LinkerSymbol;
    pub unsafe static __irq_140_stub: LinkerSymbol;
    pub unsafe static __irq_141_stub: LinkerSymbol;
    pub unsafe static __irq_142_stub: LinkerSymbol;
    pub unsafe static __irq_143_stub: LinkerSymbol;
    pub unsafe static __irq_144_stub: LinkerSymbol;
    pub unsafe static __irq_145_stub: LinkerSymbol;
    pub unsafe static __irq_146_stub: LinkerSymbol;
    pub unsafe static __irq_147_stub: LinkerSymbol;
    pub unsafe static __irq_148_stub: LinkerSymbol;
    pub unsafe static __irq_149_stub: LinkerSymbol;
    pub unsafe static __irq_150_stub: LinkerSymbol;
    pub unsafe static __irq_151_stub: LinkerSymbol;
    pub unsafe static __irq_152_stub: LinkerSymbol;
    pub unsafe static __irq_153_stub: LinkerSymbol;
    pub unsafe static __irq_154_stub: LinkerSymbol;
    pub unsafe static __irq_155_stub: LinkerSymbol;
    pub unsafe static __irq_156_stub: LinkerSymbol;
    pub unsafe static __irq_157_stub: LinkerSymbol;
    pub unsafe static __irq_158_stub: LinkerSymbol;
    pub unsafe static __irq_159_stub: LinkerSymbol;
    pub unsafe static __irq_160_stub: LinkerSymbol;
    pub unsafe static __irq_161_stub: LinkerSymbol;
    pub unsafe static __irq_162_stub: LinkerSymbol;
    pub unsafe static __irq_163_stub: LinkerSymbol;
    pub unsafe static __irq_164_stub: LinkerSymbol;
    pub unsafe static __irq_165_stub: LinkerSymbol;
    pub unsafe static __irq_166_stub: LinkerSymbol;
    pub unsafe static __irq_167_stub: LinkerSymbol;
    pub unsafe static __irq_168_stub: LinkerSymbol;
    pub unsafe static __irq_169_stub: LinkerSymbol;
    pub unsafe static __irq_170_stub: LinkerSymbol;
    pub unsafe static __irq_171_stub: LinkerSymbol;
    pub unsafe static __irq_172_stub: LinkerSymbol;
    pub unsafe static __irq_173_stub: LinkerSymbol;
    pub unsafe static __irq_174_stub: LinkerSymbol;
    pub unsafe static __irq_175_stub: LinkerSymbol;
    pub unsafe static __irq_176_stub: LinkerSymbol;
    pub unsafe static __irq_177_stub: LinkerSymbol;
    pub unsafe static __irq_178_stub: LinkerSymbol;
    pub unsafe static __irq_179_stub: LinkerSymbol;
    pub unsafe static __irq_180_stub: LinkerSymbol;
    pub unsafe static __irq_181_stub: LinkerSymbol;
    pub unsafe static __irq_182_stub: LinkerSymbol;
    pub unsafe static __irq_183_stub: LinkerSymbol;
    pub unsafe static __irq_184_stub: LinkerSymbol;
    pub unsafe static __irq_185_stub: LinkerSymbol;
    pub unsafe static __irq_186_stub: LinkerSymbol;
    pub unsafe static __irq_187_stub: LinkerSymbol;
    pub unsafe static __irq_188_stub: LinkerSymbol;
    pub unsafe static __irq_189_stub: LinkerSymbol;
    pub unsafe static __irq_190_stub: LinkerSymbol;
    pub unsafe static __irq_191_stub: LinkerSymbol;
    pub unsafe static __irq_192_stub: LinkerSymbol;
    pub unsafe static __irq_193_stub: LinkerSymbol;
    pub unsafe static __irq_194_stub: LinkerSymbol;
    pub unsafe static __irq_195_stub: LinkerSymbol;
    pub unsafe static __irq_196_stub: LinkerSymbol;
    pub unsafe static __irq_197_stub: LinkerSymbol;
    pub unsafe static __irq_198_stub: LinkerSymbol;
    pub unsafe static __irq_199_stub: LinkerSymbol;
    pub unsafe static __irq_200_stub: LinkerSymbol;
    pub unsafe static __irq_201_stub: LinkerSymbol;
    pub unsafe static __irq_202_stub: LinkerSymbol;
    pub unsafe static __irq_203_stub: LinkerSymbol;
    pub unsafe static __irq_204_stub: LinkerSymbol;
    pub unsafe static __irq_205_stub: LinkerSymbol;
    pub unsafe static __irq_206_stub: LinkerSymbol;
    pub unsafe static __irq_207_stub: LinkerSymbol;
    pub unsafe static __irq_208_stub: LinkerSymbol;
    pub unsafe static __irq_209_stub: LinkerSymbol;
    pub unsafe static __irq_210_stub: LinkerSymbol;
    pub unsafe static __irq_211_stub: LinkerSymbol;
    pub unsafe static __irq_212_stub: LinkerSymbol;
    pub unsafe static __irq_213_stub: LinkerSymbol;
    pub unsafe static __irq_214_stub: LinkerSymbol;
    pub unsafe static __irq_215_stub: LinkerSymbol;
    pub unsafe static __irq_216_stub: LinkerSymbol;
    pub unsafe static __irq_217_stub: LinkerSymbol;
    pub unsafe static __irq_218_stub: LinkerSymbol;
    pub unsafe static __irq_219_stub: LinkerSymbol;
    pub unsafe static __irq_220_stub: LinkerSymbol;
    pub unsafe static __irq_221_stub: LinkerSymbol;
    pub unsafe static __irq_222_stub: LinkerSymbol;
    pub unsafe static __irq_223_stub: LinkerSymbol;
    pub unsafe static __irq_224_stub: LinkerSymbol;
    pub unsafe static __irq_225_stub: LinkerSymbol;
    pub unsafe static __irq_226_stub: LinkerSymbol;
    pub unsafe static __irq_227_stub: LinkerSymbol;
    pub unsafe static __irq_228_stub: LinkerSymbol;
    pub unsafe static __irq_229_stub: LinkerSymbol;
    pub unsafe static __irq_230_stub: LinkerSymbol;
    pub unsafe static __irq_231_stub: LinkerSymbol;
    pub unsafe static __irq_232_stub: LinkerSymbol;
    pub unsafe static __irq_233_stub: LinkerSymbol;
    pub unsafe static __irq_234_stub: LinkerSymbol;
    pub unsafe static __irq_235_stub: LinkerSymbol;
    pub unsafe static __irq_236_stub: LinkerSymbol;
    pub unsafe static __irq_237_stub: LinkerSymbol;
    pub unsafe static __irq_238_stub: LinkerSymbol;
    pub unsafe static __irq_239_stub: LinkerSymbol;
    pub unsafe static __irq_240_stub: LinkerSymbol;
    pub unsafe static __irq_241_stub: LinkerSymbol;
    pub unsafe static __irq_242_stub: LinkerSymbol;
    pub unsafe static __irq_243_stub: LinkerSymbol;
    pub unsafe static __irq_244_stub: LinkerSymbol;
    pub unsafe static __irq_245_stub: LinkerSymbol;
    pub unsafe static __irq_246_stub: LinkerSymbol;
    pub unsafe static __irq_247_stub: LinkerSymbol;
    pub unsafe static __irq_248_stub: LinkerSymbol;
    pub unsafe static __irq_249_stub: LinkerSymbol;
    pub unsafe static __irq_250_stub: LinkerSymbol;
    pub unsafe static __irq_251_stub: LinkerSymbol;
    pub unsafe static __irq_252_stub: LinkerSymbol;
    pub unsafe static __irq_253_stub: LinkerSymbol;
    pub unsafe static __irq_254_stub: LinkerSymbol;
    pub unsafe static __irq_255_stub: LinkerSymbol;
}

core::arch::global_asm! {
"
.global __de_stub
__de_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __de_handler
  add rsp, 0x10
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
  iretq

.global __db_stub
__db_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __db_handler
  add rsp, 0x10
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
  iretq

.global __nm_stub
__nm_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __nm_handler
  add rsp, 0x10
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
  iretq

.global __bp_stub
__bp_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __bp_handler
  add rsp, 0x10
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
  iretq

.global __of_stub
__of_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __of_handler
  add rsp, 0x10
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
  iretq

.global __br_stub
__br_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __br_handler
  add rsp, 0x10
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
  iretq

.global __ud_stub
__ud_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __ud_handler
  add rsp, 0x10
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
  iretq

.global __na_stub
__na_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __na_handler
  add rsp, 0x10
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
  iretq

.global __mf_stub
__mf_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __mf_handler
  add rsp, 0x10
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
  iretq

.global __xm_stub
__xm_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __xm_handler
  add rsp, 0x10
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
  iretq

.global __ve_stub
__ve_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __ve_handler
  add rsp, 0x10
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
  iretq

.global __ts_stub
__ts_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __ts_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __np_stub
__np_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __np_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __ss_stub
__ss_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __ss_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __gp_stub
__gp_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __gp_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __pf_stub
__pf_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __pf_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __ac_stub
__ac_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __ac_handler
  add rsp, 0x18
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
  add rsp, 0x8
  iretq

.global __mc_stub
__mc_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (17 * 8)]
  lea rsi, [rsp + (2 * 8)]
  call __mc_handler
  add rsp, 0x10
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
  2:
  pause
  jmp 2b

.global __df_stub
__df_stub:
  cld
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
  mov rax, [rsp + ((16 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((16 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  lea rdi, [rsp + (18 * 8)]
  mov rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  sub rsp, 0x8
  call __df_handler
  add rsp, 0x18
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
  add rsp, 0x8
  2:
  pause
  jmp 2b

.global __irq_32_stub
__irq_32_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 32
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_33_stub
__irq_33_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 33
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_34_stub
__irq_34_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 34
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_35_stub
__irq_35_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 35
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_36_stub
__irq_36_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 36
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_37_stub
__irq_37_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 37
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_38_stub
__irq_38_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 38
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_39_stub
__irq_39_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 39
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_40_stub
__irq_40_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 40
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_41_stub
__irq_41_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 41
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_42_stub
__irq_42_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 42
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_43_stub
__irq_43_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 43
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_44_stub
__irq_44_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 44
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_45_stub
__irq_45_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 45
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_46_stub
__irq_46_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 46
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_47_stub
__irq_47_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 47
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_48_stub
__irq_48_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 48
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_49_stub
__irq_49_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 49
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_50_stub
__irq_50_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 50
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_51_stub
__irq_51_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 51
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_52_stub
__irq_52_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 52
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_53_stub
__irq_53_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 53
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_54_stub
__irq_54_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 54
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_55_stub
__irq_55_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 55
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_56_stub
__irq_56_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 56
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_57_stub
__irq_57_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 57
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_58_stub
__irq_58_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 58
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_59_stub
__irq_59_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 59
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_60_stub
__irq_60_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 60
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_61_stub
__irq_61_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 61
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_62_stub
__irq_62_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 62
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_63_stub
__irq_63_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 63
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_64_stub
__irq_64_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 64
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_65_stub
__irq_65_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 65
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_66_stub
__irq_66_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 66
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_67_stub
__irq_67_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 67
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_68_stub
__irq_68_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 68
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_69_stub
__irq_69_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 69
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_70_stub
__irq_70_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 70
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_71_stub
__irq_71_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 71
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_72_stub
__irq_72_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 72
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_73_stub
__irq_73_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 73
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_74_stub
__irq_74_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 74
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_75_stub
__irq_75_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 75
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_76_stub
__irq_76_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 76
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_77_stub
__irq_77_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 77
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_78_stub
__irq_78_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 78
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_79_stub
__irq_79_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 79
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_80_stub
__irq_80_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 80
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_81_stub
__irq_81_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 81
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_82_stub
__irq_82_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 82
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_83_stub
__irq_83_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 83
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_84_stub
__irq_84_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 84
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_85_stub
__irq_85_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 85
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_86_stub
__irq_86_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 86
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_87_stub
__irq_87_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 87
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_88_stub
__irq_88_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 88
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_89_stub
__irq_89_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 89
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_90_stub
__irq_90_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 90
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_91_stub
__irq_91_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 91
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_92_stub
__irq_92_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 92
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_93_stub
__irq_93_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 93
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_94_stub
__irq_94_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 94
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_95_stub
__irq_95_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 95
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_96_stub
__irq_96_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 96
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_97_stub
__irq_97_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 97
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_98_stub
__irq_98_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 98
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_99_stub
__irq_99_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 99
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_100_stub
__irq_100_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 100
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_101_stub
__irq_101_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 101
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_102_stub
__irq_102_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 102
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_103_stub
__irq_103_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 103
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_104_stub
__irq_104_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 104
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_105_stub
__irq_105_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 105
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_106_stub
__irq_106_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 106
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_107_stub
__irq_107_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 107
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_108_stub
__irq_108_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 108
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_109_stub
__irq_109_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 109
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_110_stub
__irq_110_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 110
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_111_stub
__irq_111_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 111
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_112_stub
__irq_112_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 112
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_113_stub
__irq_113_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 113
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_114_stub
__irq_114_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 114
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_115_stub
__irq_115_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 115
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_116_stub
__irq_116_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 116
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_117_stub
__irq_117_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 117
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_118_stub
__irq_118_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 118
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_119_stub
__irq_119_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 119
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_120_stub
__irq_120_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 120
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_121_stub
__irq_121_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 121
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_122_stub
__irq_122_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 122
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_123_stub
__irq_123_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 123
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_124_stub
__irq_124_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 124
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_125_stub
__irq_125_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 125
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_126_stub
__irq_126_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 126
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_127_stub
__irq_127_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 127
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_128_stub
__irq_128_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 128
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_129_stub
__irq_129_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 129
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_130_stub
__irq_130_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 130
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_131_stub
__irq_131_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 131
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_132_stub
__irq_132_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 132
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_133_stub
__irq_133_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 133
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_134_stub
__irq_134_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 134
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_135_stub
__irq_135_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 135
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_136_stub
__irq_136_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 136
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_137_stub
__irq_137_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 137
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_138_stub
__irq_138_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 138
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_139_stub
__irq_139_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 139
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_140_stub
__irq_140_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 140
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_141_stub
__irq_141_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 141
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_142_stub
__irq_142_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 142
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_143_stub
__irq_143_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 143
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_144_stub
__irq_144_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 144
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_145_stub
__irq_145_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 145
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_146_stub
__irq_146_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 146
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_147_stub
__irq_147_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 147
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_148_stub
__irq_148_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 148
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_149_stub
__irq_149_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 149
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_150_stub
__irq_150_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 150
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_151_stub
__irq_151_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 151
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_152_stub
__irq_152_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 152
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_153_stub
__irq_153_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 153
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_154_stub
__irq_154_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 154
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_155_stub
__irq_155_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 155
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_156_stub
__irq_156_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 156
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_157_stub
__irq_157_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 157
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_158_stub
__irq_158_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 158
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_159_stub
__irq_159_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 159
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_160_stub
__irq_160_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 160
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_161_stub
__irq_161_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 161
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_162_stub
__irq_162_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 162
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_163_stub
__irq_163_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 163
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_164_stub
__irq_164_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 164
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_165_stub
__irq_165_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 165
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_166_stub
__irq_166_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 166
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_167_stub
__irq_167_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 167
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_168_stub
__irq_168_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 168
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_169_stub
__irq_169_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 169
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_170_stub
__irq_170_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 170
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_171_stub
__irq_171_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 171
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_172_stub
__irq_172_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 172
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_173_stub
__irq_173_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 173
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_174_stub
__irq_174_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 174
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_175_stub
__irq_175_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 175
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_176_stub
__irq_176_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 176
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_177_stub
__irq_177_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 177
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_178_stub
__irq_178_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 178
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_179_stub
__irq_179_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 179
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_180_stub
__irq_180_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 180
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_181_stub
__irq_181_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 181
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_182_stub
__irq_182_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 182
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_183_stub
__irq_183_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 183
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_184_stub
__irq_184_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 184
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_185_stub
__irq_185_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 185
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_186_stub
__irq_186_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 186
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_187_stub
__irq_187_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 187
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_188_stub
__irq_188_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 188
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_189_stub
__irq_189_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 189
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_190_stub
__irq_190_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 190
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_191_stub
__irq_191_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 191
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_192_stub
__irq_192_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 192
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_193_stub
__irq_193_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 193
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_194_stub
__irq_194_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 194
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_195_stub
__irq_195_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 195
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_196_stub
__irq_196_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 196
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_197_stub
__irq_197_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 197
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_198_stub
__irq_198_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 198
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_199_stub
__irq_199_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 199
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_200_stub
__irq_200_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 200
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_201_stub
__irq_201_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 201
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_202_stub
__irq_202_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 202
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_203_stub
__irq_203_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 203
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_204_stub
__irq_204_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 204
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_205_stub
__irq_205_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 205
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_206_stub
__irq_206_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 206
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_207_stub
__irq_207_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 207
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_208_stub
__irq_208_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 208
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_209_stub
__irq_209_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 209
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_210_stub
__irq_210_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 210
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_211_stub
__irq_211_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 211
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_212_stub
__irq_212_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 212
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_213_stub
__irq_213_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 213
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_214_stub
__irq_214_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 214
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_215_stub
__irq_215_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 215
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_216_stub
__irq_216_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 216
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_217_stub
__irq_217_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 217
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_218_stub
__irq_218_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 218
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_219_stub
__irq_219_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 219
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_220_stub
__irq_220_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 220
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_221_stub
__irq_221_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 221
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_222_stub
__irq_222_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 222
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_223_stub
__irq_223_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 223
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_224_stub
__irq_224_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 224
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_225_stub
__irq_225_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 225
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_226_stub
__irq_226_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 226
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_227_stub
__irq_227_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 227
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_228_stub
__irq_228_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 228
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_229_stub
__irq_229_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 229
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_230_stub
__irq_230_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 230
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_231_stub
__irq_231_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 231
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_232_stub
__irq_232_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 232
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_233_stub
__irq_233_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 233
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_234_stub
__irq_234_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 234
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_235_stub
__irq_235_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 235
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_236_stub
__irq_236_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 236
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_237_stub
__irq_237_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 237
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_238_stub
__irq_238_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 238
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_239_stub
__irq_239_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 239
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_240_stub
__irq_240_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 240
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_241_stub
__irq_241_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 241
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_242_stub
__irq_242_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 242
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_243_stub
__irq_243_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 243
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_244_stub
__irq_244_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 244
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_245_stub
__irq_245_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 245
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_246_stub
__irq_246_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 246
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_247_stub
__irq_247_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 247
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_248_stub
__irq_248_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 248
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_249_stub
__irq_249_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 249
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_250_stub
__irq_250_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 250
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_251_stub
__irq_251_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 251
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_252_stub
__irq_252_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 252
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_253_stub
__irq_253_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 253
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_254_stub
__irq_254_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 254
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq

.global __irq_255_stub
__irq_255_stub:
  cld
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
  mov rax, [rsp + ((15 + 1) * 0)]
  cmp rax, 0x8
  je 2f
  xor rbp, rbp
  2:
  mov rax, [rsp + ((15 + 1) * 8)]
  push rax
  push rbp
  mov rbp, rsp
  mov rdi, 255
  lea rsi, [rsp + (17 * 8)]
  lea rdx, [rsp + (2 * 8)]
  call __irq_handler
  add rsp, 0x10
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
  iretq
"
}
