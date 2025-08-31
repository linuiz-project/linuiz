use crate::{
    LinkerSymbol,
    arch::x86_64::{
        devices::local_apic::LocalApic,
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
    handle(ArchException::DivideError(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __db_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::Debug(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __nm_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::NonMaskable(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __bp_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::Breakpoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __of_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::Overflow(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __br_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::BoundRangeExceeded(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ud_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::InvalidOpcode(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __na_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::DeviceNotAvailable(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __df_handler(stack_frame: &InterruptStackFrame, _: u64, gprs: &Registers) {
    handle(ArchException::DoubleFault(stack_frame, gprs));
    unreachable!("#DF cannot be recovered from");
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ts_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(ArchException::InvalidTSS(
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
    handle(ArchException::SegmentNotPresent(
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
    handle(ArchException::StackSegmentFault(
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
    handle(ArchException::GeneralProtectionFault(
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
    handle(ArchException::PageFault(
        stack_frame,
        gprs,
        err,
        crate::arch::x86_64::registers::control::cr2::CR2::read(),
    ));
}

// --- reserved 15

#[unsafe(no_mangle)]
extern "sysv64" fn __mf_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::x87FloatingPoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ac_handler(
    stack_frame: &InterruptStackFrame,
    error_code: u64,
    gprs: &Registers,
) {
    handle(ArchException::AlignmentCheck(stack_frame, error_code, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __mc_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::MachineCheck(stack_frame, gprs));
    unreachable!("#MC cannot be recovered");
}

#[unsafe(no_mangle)]
extern "sysv64" fn __xm_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::SimdFlaotingPoint(stack_frame, gprs));
}

#[unsafe(no_mangle)]
extern "sysv64" fn __ve_handler(stack_frame: &InterruptStackFrame, gprs: &Registers) {
    handle(ArchException::Virtualization(stack_frame, gprs));
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
    match Vector::try_from(irq_number) {
        Ok(Vector::Timer) => {
            LocalState::with_scheduler(|scheduler| {
                scheduler.interrupt_task(isf, regs);
            });
        }

        Ok(Vector::Syscall) => {
            let result = crate::interrupts::syscall::process(
                regs.rsi, regs.rdi, regs.rax, regs.rcx, regs.rdx, isf, regs,
            );
            trace!("{result:#X?}");

            regs.rdi = result.code.map_or(0, core::num::NonZero::get);
            regs.rsi = result.value;
        }

        vector => unimplemented!("unsupported interrupt vector: {vector:?}"),
    }

    // Safety: This is the end of an interrupt service routine.
    unsafe {
        LocalApic::end_of_interrupt();
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
