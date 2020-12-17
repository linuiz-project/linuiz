use crate::structures::{DescriptorTablePointer, gdt::segment_selector::SegmentSelector};

pub unsafe fn lgdt(gdt: &DescriptorTablePointer) {
    asm!("lgdt [{}]", in(reg) gdt, options(nostack));
}

pub unsafe fn load_tss(sel: SegmentSelector) {
    asm!("ltr {0:x}", in(reg) sel.0, options(nostack, nomem));
}