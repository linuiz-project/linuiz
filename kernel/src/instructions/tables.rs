pub unsafe fn lgdt(gdt: &crate::structures::DescriptorTablePointer) {
    asm!("lgdt [{}]", in(reg) gdt, options(nostack));
}

pub unsafe fn load_tss(sel: crate::structures::gdt::SegmentSelector) {
    asm!("ltr {0:x}", in(reg) sel.0, options(nostack, nomem));
}
