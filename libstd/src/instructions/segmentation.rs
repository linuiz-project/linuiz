use core::arch::asm;

#[inline]
pub unsafe fn lgdt(pointer: &crate::structures::gdt::Pointer) {
    asm!("lgdt [{}]", in(reg) pointer, options(readonly, nostack, preserves_flags));
}

#[inline]
pub unsafe fn ltr(segment_index: u16) {
    asm!("ltr {:x}", in(reg) segment_index, options(nomem, nostack, preserves_flags));
}
