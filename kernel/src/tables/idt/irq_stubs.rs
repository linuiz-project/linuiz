use super::InterruptStackFrame;
use core::arch::asm;

pub fn apply_stubs(idt: &mut spin::MutexGuard<super::InterruptDescriptorTable>) {
    idt[32].set_handler_fn(irq_32);
    idt[33].set_handler_fn(irq_33);
    idt[34].set_handler_fn(irq_34);
    idt[35].set_handler_fn(irq_35);
    idt[36].set_handler_fn(irq_36);
    idt[37].set_handler_fn(irq_37);
    idt[38].set_handler_fn(irq_38);
    idt[39].set_handler_fn(irq_39);
    idt[40].set_handler_fn(irq_40);
    idt[41].set_handler_fn(irq_41);
    idt[42].set_handler_fn(irq_42);
    idt[43].set_handler_fn(irq_43);
    idt[44].set_handler_fn(irq_44);
    idt[45].set_handler_fn(irq_45);
    idt[46].set_handler_fn(irq_46);
    idt[47].set_handler_fn(irq_47);
    idt[48].set_handler_fn(irq_48);
    idt[49].set_handler_fn(irq_49);
    idt[50].set_handler_fn(irq_50);
    idt[51].set_handler_fn(irq_51);
    idt[52].set_handler_fn(irq_52);
    idt[53].set_handler_fn(irq_53);
    idt[54].set_handler_fn(irq_54);
    idt[55].set_handler_fn(irq_55);
    idt[56].set_handler_fn(irq_56);
    idt[57].set_handler_fn(irq_57);
    idt[58].set_handler_fn(irq_58);
    idt[59].set_handler_fn(irq_59);
    idt[60].set_handler_fn(irq_60);
    idt[61].set_handler_fn(irq_61);
    idt[62].set_handler_fn(irq_62);
    idt[63].set_handler_fn(irq_63);
    idt[64].set_handler_fn(irq_64);
    idt[65].set_handler_fn(irq_65);
    idt[66].set_handler_fn(irq_66);
    idt[67].set_handler_fn(irq_67);
    idt[68].set_handler_fn(irq_68);
    idt[69].set_handler_fn(irq_69);
    idt[70].set_handler_fn(irq_70);
    idt[71].set_handler_fn(irq_71);
    idt[72].set_handler_fn(irq_72);
    idt[73].set_handler_fn(irq_73);
    idt[74].set_handler_fn(irq_74);
    idt[75].set_handler_fn(irq_75);
    idt[76].set_handler_fn(irq_76);
    idt[77].set_handler_fn(irq_77);
    idt[78].set_handler_fn(irq_78);
    idt[79].set_handler_fn(irq_79);
    idt[80].set_handler_fn(irq_80);
    idt[81].set_handler_fn(irq_81);
    idt[82].set_handler_fn(irq_82);
    idt[83].set_handler_fn(irq_83);
    idt[84].set_handler_fn(irq_84);
    idt[85].set_handler_fn(irq_85);
    idt[86].set_handler_fn(irq_86);
    idt[87].set_handler_fn(irq_87);
    idt[88].set_handler_fn(irq_88);
    idt[89].set_handler_fn(irq_89);
    idt[90].set_handler_fn(irq_90);
    idt[91].set_handler_fn(irq_91);
    idt[92].set_handler_fn(irq_92);
    idt[93].set_handler_fn(irq_93);
    idt[94].set_handler_fn(irq_94);
    idt[95].set_handler_fn(irq_95);
    idt[96].set_handler_fn(irq_96);
    idt[97].set_handler_fn(irq_97);
    idt[98].set_handler_fn(irq_98);
    idt[99].set_handler_fn(irq_99);
    idt[100].set_handler_fn(irq_100);
    idt[101].set_handler_fn(irq_101);
    idt[102].set_handler_fn(irq_102);
    idt[103].set_handler_fn(irq_103);
    idt[104].set_handler_fn(irq_104);
    idt[105].set_handler_fn(irq_105);
    idt[106].set_handler_fn(irq_106);
    idt[107].set_handler_fn(irq_107);
    idt[108].set_handler_fn(irq_108);
    idt[109].set_handler_fn(irq_109);
    idt[110].set_handler_fn(irq_110);
    idt[111].set_handler_fn(irq_111);
    idt[112].set_handler_fn(irq_112);
    idt[113].set_handler_fn(irq_113);
    idt[114].set_handler_fn(irq_114);
    idt[115].set_handler_fn(irq_115);
    idt[116].set_handler_fn(irq_116);
    idt[117].set_handler_fn(irq_117);
    idt[118].set_handler_fn(irq_118);
    idt[119].set_handler_fn(irq_119);
    idt[120].set_handler_fn(irq_120);
    idt[121].set_handler_fn(irq_121);
    idt[122].set_handler_fn(irq_122);
    idt[123].set_handler_fn(irq_123);
    idt[124].set_handler_fn(irq_124);
    idt[125].set_handler_fn(irq_125);
    idt[126].set_handler_fn(irq_126);
    idt[127].set_handler_fn(irq_127);
    idt[128].set_handler_fn(irq_128);
    idt[129].set_handler_fn(irq_129);
    idt[130].set_handler_fn(irq_130);
    idt[131].set_handler_fn(irq_131);
    idt[132].set_handler_fn(irq_132);
    idt[133].set_handler_fn(irq_133);
    idt[134].set_handler_fn(irq_134);
    idt[135].set_handler_fn(irq_135);
    idt[136].set_handler_fn(irq_136);
    idt[137].set_handler_fn(irq_137);
    idt[138].set_handler_fn(irq_138);
    idt[139].set_handler_fn(irq_139);
    idt[140].set_handler_fn(irq_140);
    idt[141].set_handler_fn(irq_141);
    idt[142].set_handler_fn(irq_142);
    idt[143].set_handler_fn(irq_143);
    idt[144].set_handler_fn(irq_144);
    idt[145].set_handler_fn(irq_145);
    idt[146].set_handler_fn(irq_146);
    idt[147].set_handler_fn(irq_147);
    idt[148].set_handler_fn(irq_148);
    idt[149].set_handler_fn(irq_149);
    idt[150].set_handler_fn(irq_150);
    idt[151].set_handler_fn(irq_151);
    idt[152].set_handler_fn(irq_152);
    idt[153].set_handler_fn(irq_153);
    idt[154].set_handler_fn(irq_154);
    idt[155].set_handler_fn(irq_155);
    idt[156].set_handler_fn(irq_156);
    idt[157].set_handler_fn(irq_157);
    idt[158].set_handler_fn(irq_158);
    idt[159].set_handler_fn(irq_159);
    idt[160].set_handler_fn(irq_160);
    idt[161].set_handler_fn(irq_161);
    idt[162].set_handler_fn(irq_162);
    idt[163].set_handler_fn(irq_163);
    idt[164].set_handler_fn(irq_164);
    idt[165].set_handler_fn(irq_165);
    idt[166].set_handler_fn(irq_166);
    idt[167].set_handler_fn(irq_167);
    idt[168].set_handler_fn(irq_168);
    idt[169].set_handler_fn(irq_169);
    idt[170].set_handler_fn(irq_170);
    idt[171].set_handler_fn(irq_171);
    idt[172].set_handler_fn(irq_172);
    idt[173].set_handler_fn(irq_173);
    idt[174].set_handler_fn(irq_174);
    idt[175].set_handler_fn(irq_175);
    idt[176].set_handler_fn(irq_176);
    idt[177].set_handler_fn(irq_177);
    idt[178].set_handler_fn(irq_178);
    idt[179].set_handler_fn(irq_179);
    idt[180].set_handler_fn(irq_180);
    idt[181].set_handler_fn(irq_181);
    idt[182].set_handler_fn(irq_182);
    idt[183].set_handler_fn(irq_183);
    idt[184].set_handler_fn(irq_184);
    idt[185].set_handler_fn(irq_185);
    idt[186].set_handler_fn(irq_186);
    idt[187].set_handler_fn(irq_187);
    idt[188].set_handler_fn(irq_188);
    idt[189].set_handler_fn(irq_189);
    idt[190].set_handler_fn(irq_190);
    idt[191].set_handler_fn(irq_191);
    idt[192].set_handler_fn(irq_192);
    idt[193].set_handler_fn(irq_193);
    idt[194].set_handler_fn(irq_194);
    idt[195].set_handler_fn(irq_195);
    idt[196].set_handler_fn(irq_196);
    idt[197].set_handler_fn(irq_197);
    idt[198].set_handler_fn(irq_198);
    idt[199].set_handler_fn(irq_199);
    idt[200].set_handler_fn(irq_200);
    idt[201].set_handler_fn(irq_201);
    idt[202].set_handler_fn(irq_202);
    idt[203].set_handler_fn(irq_203);
    idt[204].set_handler_fn(irq_204);
    idt[205].set_handler_fn(irq_205);
    idt[206].set_handler_fn(irq_206);
    idt[207].set_handler_fn(irq_207);
    idt[208].set_handler_fn(irq_208);
    idt[209].set_handler_fn(irq_209);
    idt[210].set_handler_fn(irq_210);
    idt[211].set_handler_fn(irq_211);
    idt[212].set_handler_fn(irq_212);
    idt[213].set_handler_fn(irq_213);
    idt[214].set_handler_fn(irq_214);
    idt[215].set_handler_fn(irq_215);
    idt[216].set_handler_fn(irq_216);
    idt[217].set_handler_fn(irq_217);
    idt[218].set_handler_fn(irq_218);
    idt[219].set_handler_fn(irq_219);
    idt[220].set_handler_fn(irq_220);
    idt[221].set_handler_fn(irq_221);
    idt[222].set_handler_fn(irq_222);
    idt[223].set_handler_fn(irq_223);
    idt[224].set_handler_fn(irq_224);
    idt[225].set_handler_fn(irq_225);
    idt[226].set_handler_fn(irq_226);
    idt[227].set_handler_fn(irq_227);
    idt[228].set_handler_fn(irq_228);
    idt[229].set_handler_fn(irq_229);
    idt[230].set_handler_fn(irq_230);
    idt[231].set_handler_fn(irq_231);
    idt[232].set_handler_fn(irq_232);
    idt[233].set_handler_fn(irq_233);
    idt[234].set_handler_fn(irq_234);
    idt[235].set_handler_fn(irq_235);
    idt[236].set_handler_fn(irq_236);
    idt[237].set_handler_fn(irq_237);
    idt[238].set_handler_fn(irq_238);
    idt[239].set_handler_fn(irq_239);
    idt[240].set_handler_fn(irq_240);
    idt[241].set_handler_fn(irq_241);
    idt[242].set_handler_fn(irq_242);
    idt[243].set_handler_fn(irq_243);
    idt[244].set_handler_fn(irq_244);
    idt[245].set_handler_fn(irq_245);
    idt[246].set_handler_fn(irq_246);
    idt[247].set_handler_fn(irq_247);
    idt[248].set_handler_fn(irq_248);
    idt[249].set_handler_fn(irq_249);
    idt[250].set_handler_fn(irq_250);
    idt[251].set_handler_fn(irq_251);
    idt[252].set_handler_fn(irq_252);
    idt[253].set_handler_fn(irq_253);
    idt[254].set_handler_fn(irq_254);
    idt[255].set_handler_fn(irq_255);
}

#[naked]
extern "x86-interrupt" fn irq_32(_: InterruptStackFrame) {
    unsafe {
        asm!("push $32", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_33(_: InterruptStackFrame) {
    unsafe {
        asm!("push $33", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_34(_: InterruptStackFrame) {
    unsafe {
        asm!("push $34", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_35(_: InterruptStackFrame) {
    unsafe {
        asm!("push $35", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_36(_: InterruptStackFrame) {
    unsafe {
        asm!("push $36", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_37(_: InterruptStackFrame) {
    unsafe {
        asm!("push $37", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_38(_: InterruptStackFrame) {
    unsafe {
        asm!("push $38", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_39(_: InterruptStackFrame) {
    unsafe {
        asm!("push $39", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_40(_: InterruptStackFrame) {
    unsafe {
        asm!("push $40", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_41(_: InterruptStackFrame) {
    unsafe {
        asm!("push $41", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_42(_: InterruptStackFrame) {
    unsafe {
        asm!("push $42", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_43(_: InterruptStackFrame) {
    unsafe {
        asm!("push $43", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_44(_: InterruptStackFrame) {
    unsafe {
        asm!("push $44", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_45(_: InterruptStackFrame) {
    unsafe {
        asm!("push $45", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_46(_: InterruptStackFrame) {
    unsafe {
        asm!("push $46", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_47(_: InterruptStackFrame) {
    unsafe {
        asm!("push $47", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_48(_: InterruptStackFrame) {
    unsafe {
        asm!("push $48", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_49(_: InterruptStackFrame) {
    unsafe {
        asm!("push $49", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_50(_: InterruptStackFrame) {
    unsafe {
        asm!("push $50", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_51(_: InterruptStackFrame) {
    unsafe {
        asm!("push $51", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_52(_: InterruptStackFrame) {
    unsafe {
        asm!("push $52", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_53(_: InterruptStackFrame) {
    unsafe {
        asm!("push $53", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_54(_: InterruptStackFrame) {
    unsafe {
        asm!("push $54", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_55(_: InterruptStackFrame) {
    unsafe {
        asm!("push $55", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_56(_: InterruptStackFrame) {
    unsafe {
        asm!("push $56", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_57(_: InterruptStackFrame) {
    unsafe {
        asm!("push $57", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_58(_: InterruptStackFrame) {
    unsafe {
        asm!("push $58", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_59(_: InterruptStackFrame) {
    unsafe {
        asm!("push $59", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_60(_: InterruptStackFrame) {
    unsafe {
        asm!("push $60", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_61(_: InterruptStackFrame) {
    unsafe {
        asm!("push $61", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_62(_: InterruptStackFrame) {
    unsafe {
        asm!("push $62", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_63(_: InterruptStackFrame) {
    unsafe {
        asm!("push $63", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_64(_: InterruptStackFrame) {
    unsafe {
        asm!("push $64", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_65(_: InterruptStackFrame) {
    unsafe {
        asm!("push $65", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_66(_: InterruptStackFrame) {
    unsafe {
        asm!("push $66", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_67(_: InterruptStackFrame) {
    unsafe {
        asm!("push $67", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_68(_: InterruptStackFrame) {
    unsafe {
        asm!("push $68", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_69(_: InterruptStackFrame) {
    unsafe {
        asm!("push $69", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_70(_: InterruptStackFrame) {
    unsafe {
        asm!("push $70", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_71(_: InterruptStackFrame) {
    unsafe {
        asm!("push $71", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_72(_: InterruptStackFrame) {
    unsafe {
        asm!("push $72", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_73(_: InterruptStackFrame) {
    unsafe {
        asm!("push $73", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_74(_: InterruptStackFrame) {
    unsafe {
        asm!("push $74", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_75(_: InterruptStackFrame) {
    unsafe {
        asm!("push $75", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_76(_: InterruptStackFrame) {
    unsafe {
        asm!("push $76", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_77(_: InterruptStackFrame) {
    unsafe {
        asm!("push $77", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_78(_: InterruptStackFrame) {
    unsafe {
        asm!("push $78", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_79(_: InterruptStackFrame) {
    unsafe {
        asm!("push $79", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_80(_: InterruptStackFrame) {
    unsafe {
        asm!("push $80", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_81(_: InterruptStackFrame) {
    unsafe {
        asm!("push $81", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_82(_: InterruptStackFrame) {
    unsafe {
        asm!("push $82", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_83(_: InterruptStackFrame) {
    unsafe {
        asm!("push $83", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_84(_: InterruptStackFrame) {
    unsafe {
        asm!("push $84", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_85(_: InterruptStackFrame) {
    unsafe {
        asm!("push $85", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_86(_: InterruptStackFrame) {
    unsafe {
        asm!("push $86", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_87(_: InterruptStackFrame) {
    unsafe {
        asm!("push $87", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_88(_: InterruptStackFrame) {
    unsafe {
        asm!("push $88", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_89(_: InterruptStackFrame) {
    unsafe {
        asm!("push $89", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_90(_: InterruptStackFrame) {
    unsafe {
        asm!("push $90", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_91(_: InterruptStackFrame) {
    unsafe {
        asm!("push $91", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_92(_: InterruptStackFrame) {
    unsafe {
        asm!("push $92", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_93(_: InterruptStackFrame) {
    unsafe {
        asm!("push $93", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_94(_: InterruptStackFrame) {
    unsafe {
        asm!("push $94", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_95(_: InterruptStackFrame) {
    unsafe {
        asm!("push $95", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_96(_: InterruptStackFrame) {
    unsafe {
        asm!("push $96", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_97(_: InterruptStackFrame) {
    unsafe {
        asm!("push $97", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_98(_: InterruptStackFrame) {
    unsafe {
        asm!("push $98", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_99(_: InterruptStackFrame) {
    unsafe {
        asm!("push $99", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_100(_: InterruptStackFrame) {
    unsafe {
        asm!("push $100", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_101(_: InterruptStackFrame) {
    unsafe {
        asm!("push $101", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_102(_: InterruptStackFrame) {
    unsafe {
        asm!("push $102", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_103(_: InterruptStackFrame) {
    unsafe {
        asm!("push $103", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_104(_: InterruptStackFrame) {
    unsafe {
        asm!("push $104", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_105(_: InterruptStackFrame) {
    unsafe {
        asm!("push $105", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_106(_: InterruptStackFrame) {
    unsafe {
        asm!("push $106", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_107(_: InterruptStackFrame) {
    unsafe {
        asm!("push $107", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_108(_: InterruptStackFrame) {
    unsafe {
        asm!("push $108", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_109(_: InterruptStackFrame) {
    unsafe {
        asm!("push $109", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_110(_: InterruptStackFrame) {
    unsafe {
        asm!("push $110", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_111(_: InterruptStackFrame) {
    unsafe {
        asm!("push $111", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_112(_: InterruptStackFrame) {
    unsafe {
        asm!("push $112", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_113(_: InterruptStackFrame) {
    unsafe {
        asm!("push $113", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_114(_: InterruptStackFrame) {
    unsafe {
        asm!("push $114", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_115(_: InterruptStackFrame) {
    unsafe {
        asm!("push $115", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_116(_: InterruptStackFrame) {
    unsafe {
        asm!("push $116", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_117(_: InterruptStackFrame) {
    unsafe {
        asm!("push $117", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_118(_: InterruptStackFrame) {
    unsafe {
        asm!("push $118", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_119(_: InterruptStackFrame) {
    unsafe {
        asm!("push $119", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_120(_: InterruptStackFrame) {
    unsafe {
        asm!("push $120", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_121(_: InterruptStackFrame) {
    unsafe {
        asm!("push $121", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_122(_: InterruptStackFrame) {
    unsafe {
        asm!("push $122", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_123(_: InterruptStackFrame) {
    unsafe {
        asm!("push $123", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_124(_: InterruptStackFrame) {
    unsafe {
        asm!("push $124", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_125(_: InterruptStackFrame) {
    unsafe {
        asm!("push $125", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_126(_: InterruptStackFrame) {
    unsafe {
        asm!("push $126", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_127(_: InterruptStackFrame) {
    unsafe {
        asm!("push $127", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_128(_: InterruptStackFrame) {
    unsafe {
        asm!("push $128", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_129(_: InterruptStackFrame) {
    unsafe {
        asm!("push $129", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_130(_: InterruptStackFrame) {
    unsafe {
        asm!("push $130", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_131(_: InterruptStackFrame) {
    unsafe {
        asm!("push $131", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_132(_: InterruptStackFrame) {
    unsafe {
        asm!("push $132", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_133(_: InterruptStackFrame) {
    unsafe {
        asm!("push $133", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_134(_: InterruptStackFrame) {
    unsafe {
        asm!("push $134", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_135(_: InterruptStackFrame) {
    unsafe {
        asm!("push $135", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_136(_: InterruptStackFrame) {
    unsafe {
        asm!("push $136", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_137(_: InterruptStackFrame) {
    unsafe {
        asm!("push $137", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_138(_: InterruptStackFrame) {
    unsafe {
        asm!("push $138", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_139(_: InterruptStackFrame) {
    unsafe {
        asm!("push $139", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_140(_: InterruptStackFrame) {
    unsafe {
        asm!("push $140", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_141(_: InterruptStackFrame) {
    unsafe {
        asm!("push $141", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_142(_: InterruptStackFrame) {
    unsafe {
        asm!("push $142", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_143(_: InterruptStackFrame) {
    unsafe {
        asm!("push $143", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_144(_: InterruptStackFrame) {
    unsafe {
        asm!("push $144", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_145(_: InterruptStackFrame) {
    unsafe {
        asm!("push $145", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_146(_: InterruptStackFrame) {
    unsafe {
        asm!("push $146", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_147(_: InterruptStackFrame) {
    unsafe {
        asm!("push $147", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_148(_: InterruptStackFrame) {
    unsafe {
        asm!("push $148", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_149(_: InterruptStackFrame) {
    unsafe {
        asm!("push $149", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_150(_: InterruptStackFrame) {
    unsafe {
        asm!("push $150", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_151(_: InterruptStackFrame) {
    unsafe {
        asm!("push $151", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_152(_: InterruptStackFrame) {
    unsafe {
        asm!("push $152", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_153(_: InterruptStackFrame) {
    unsafe {
        asm!("push $153", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_154(_: InterruptStackFrame) {
    unsafe {
        asm!("push $154", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_155(_: InterruptStackFrame) {
    unsafe {
        asm!("push $155", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_156(_: InterruptStackFrame) {
    unsafe {
        asm!("push $156", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_157(_: InterruptStackFrame) {
    unsafe {
        asm!("push $157", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_158(_: InterruptStackFrame) {
    unsafe {
        asm!("push $158", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_159(_: InterruptStackFrame) {
    unsafe {
        asm!("push $159", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_160(_: InterruptStackFrame) {
    unsafe {
        asm!("push $160", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_161(_: InterruptStackFrame) {
    unsafe {
        asm!("push $161", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_162(_: InterruptStackFrame) {
    unsafe {
        asm!("push $162", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_163(_: InterruptStackFrame) {
    unsafe {
        asm!("push $163", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_164(_: InterruptStackFrame) {
    unsafe {
        asm!("push $164", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_165(_: InterruptStackFrame) {
    unsafe {
        asm!("push $165", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_166(_: InterruptStackFrame) {
    unsafe {
        asm!("push $166", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_167(_: InterruptStackFrame) {
    unsafe {
        asm!("push $167", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_168(_: InterruptStackFrame) {
    unsafe {
        asm!("push $168", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_169(_: InterruptStackFrame) {
    unsafe {
        asm!("push $169", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_170(_: InterruptStackFrame) {
    unsafe {
        asm!("push $170", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_171(_: InterruptStackFrame) {
    unsafe {
        asm!("push $171", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_172(_: InterruptStackFrame) {
    unsafe {
        asm!("push $172", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_173(_: InterruptStackFrame) {
    unsafe {
        asm!("push $173", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_174(_: InterruptStackFrame) {
    unsafe {
        asm!("push $174", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_175(_: InterruptStackFrame) {
    unsafe {
        asm!("push $175", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_176(_: InterruptStackFrame) {
    unsafe {
        asm!("push $176", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_177(_: InterruptStackFrame) {
    unsafe {
        asm!("push $177", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_178(_: InterruptStackFrame) {
    unsafe {
        asm!("push $178", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_179(_: InterruptStackFrame) {
    unsafe {
        asm!("push $179", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_180(_: InterruptStackFrame) {
    unsafe {
        asm!("push $180", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_181(_: InterruptStackFrame) {
    unsafe {
        asm!("push $181", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_182(_: InterruptStackFrame) {
    unsafe {
        asm!("push $182", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_183(_: InterruptStackFrame) {
    unsafe {
        asm!("push $183", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_184(_: InterruptStackFrame) {
    unsafe {
        asm!("push $184", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_185(_: InterruptStackFrame) {
    unsafe {
        asm!("push $185", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_186(_: InterruptStackFrame) {
    unsafe {
        asm!("push $186", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_187(_: InterruptStackFrame) {
    unsafe {
        asm!("push $187", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_188(_: InterruptStackFrame) {
    unsafe {
        asm!("push $188", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_189(_: InterruptStackFrame) {
    unsafe {
        asm!("push $189", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_190(_: InterruptStackFrame) {
    unsafe {
        asm!("push $190", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_191(_: InterruptStackFrame) {
    unsafe {
        asm!("push $191", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_192(_: InterruptStackFrame) {
    unsafe {
        asm!("push $192", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_193(_: InterruptStackFrame) {
    unsafe {
        asm!("push $193", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_194(_: InterruptStackFrame) {
    unsafe {
        asm!("push $194", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_195(_: InterruptStackFrame) {
    unsafe {
        asm!("push $195", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_196(_: InterruptStackFrame) {
    unsafe {
        asm!("push $196", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_197(_: InterruptStackFrame) {
    unsafe {
        asm!("push $197", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_198(_: InterruptStackFrame) {
    unsafe {
        asm!("push $198", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_199(_: InterruptStackFrame) {
    unsafe {
        asm!("push $199", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_200(_: InterruptStackFrame) {
    unsafe {
        asm!("push $200", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_201(_: InterruptStackFrame) {
    unsafe {
        asm!("push $201", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_202(_: InterruptStackFrame) {
    unsafe {
        asm!("push $202", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_203(_: InterruptStackFrame) {
    unsafe {
        asm!("push $203", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_204(_: InterruptStackFrame) {
    unsafe {
        asm!("push $204", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_205(_: InterruptStackFrame) {
    unsafe {
        asm!("push $205", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_206(_: InterruptStackFrame) {
    unsafe {
        asm!("push $206", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_207(_: InterruptStackFrame) {
    unsafe {
        asm!("push $207", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_208(_: InterruptStackFrame) {
    unsafe {
        asm!("push $208", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_209(_: InterruptStackFrame) {
    unsafe {
        asm!("push $209", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_210(_: InterruptStackFrame) {
    unsafe {
        asm!("push $210", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_211(_: InterruptStackFrame) {
    unsafe {
        asm!("push $211", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_212(_: InterruptStackFrame) {
    unsafe {
        asm!("push $212", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_213(_: InterruptStackFrame) {
    unsafe {
        asm!("push $213", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_214(_: InterruptStackFrame) {
    unsafe {
        asm!("push $214", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_215(_: InterruptStackFrame) {
    unsafe {
        asm!("push $215", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_216(_: InterruptStackFrame) {
    unsafe {
        asm!("push $216", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_217(_: InterruptStackFrame) {
    unsafe {
        asm!("push $217", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_218(_: InterruptStackFrame) {
    unsafe {
        asm!("push $218", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_219(_: InterruptStackFrame) {
    unsafe {
        asm!("push $219", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_220(_: InterruptStackFrame) {
    unsafe {
        asm!("push $220", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_221(_: InterruptStackFrame) {
    unsafe {
        asm!("push $221", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_222(_: InterruptStackFrame) {
    unsafe {
        asm!("push $222", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_223(_: InterruptStackFrame) {
    unsafe {
        asm!("push $223", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_224(_: InterruptStackFrame) {
    unsafe {
        asm!("push $224", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_225(_: InterruptStackFrame) {
    unsafe {
        asm!("push $225", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_226(_: InterruptStackFrame) {
    unsafe {
        asm!("push $226", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_227(_: InterruptStackFrame) {
    unsafe {
        asm!("push $227", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_228(_: InterruptStackFrame) {
    unsafe {
        asm!("push $228", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_229(_: InterruptStackFrame) {
    unsafe {
        asm!("push $229", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_230(_: InterruptStackFrame) {
    unsafe {
        asm!("push $230", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_231(_: InterruptStackFrame) {
    unsafe {
        asm!("push $231", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_232(_: InterruptStackFrame) {
    unsafe {
        asm!("push $232", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_233(_: InterruptStackFrame) {
    unsafe {
        asm!("push $233", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_234(_: InterruptStackFrame) {
    unsafe {
        asm!("push $234", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_235(_: InterruptStackFrame) {
    unsafe {
        asm!("push $235", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_236(_: InterruptStackFrame) {
    unsafe {
        asm!("push $236", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_237(_: InterruptStackFrame) {
    unsafe {
        asm!("push $237", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_238(_: InterruptStackFrame) {
    unsafe {
        asm!("push $238", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_239(_: InterruptStackFrame) {
    unsafe {
        asm!("push $239", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_240(_: InterruptStackFrame) {
    unsafe {
        asm!("push $240", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_241(_: InterruptStackFrame) {
    unsafe {
        asm!("push $241", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_242(_: InterruptStackFrame) {
    unsafe {
        asm!("push $242", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_243(_: InterruptStackFrame) {
    unsafe {
        asm!("push $243", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_244(_: InterruptStackFrame) {
    unsafe {
        asm!("push $244", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_245(_: InterruptStackFrame) {
    unsafe {
        asm!("push $245", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_246(_: InterruptStackFrame) {
    unsafe {
        asm!("push $246", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_247(_: InterruptStackFrame) {
    unsafe {
        asm!("push $247", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_248(_: InterruptStackFrame) {
    unsafe {
        asm!("push $248", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_249(_: InterruptStackFrame) {
    unsafe {
        asm!("push $249", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_250(_: InterruptStackFrame) {
    unsafe {
        asm!("push $250", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_251(_: InterruptStackFrame) {
    unsafe {
        asm!("push $251", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_252(_: InterruptStackFrame) {
    unsafe {
        asm!("push $252", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_253(_: InterruptStackFrame) {
    unsafe {
        asm!("push $253", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_254(_: InterruptStackFrame) {
    unsafe {
        asm!("push $254", "call {}", sym super::irq_common, options(noreturn));
    }
}
#[naked]
extern "x86-interrupt" fn irq_255(_: InterruptStackFrame) {
    unsafe {
        asm!("push $255", "call {}", sym super::irq_common, options(noreturn));
    }
}
