use crate::structures::gdt::SegmentSelector;

pub mod interrupts;
pub mod registers;
pub mod tables;

pub fn cs() -> SegmentSelector {
    let segment: u16;
    unsafe { asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack)) };
    SegmentSelector(segment)
}

pub unsafe fn set_cs(sel: crate::structures::gdt::SegmentSelector) {
    asm!(
        "push {sel}",
        "lea {tmp}, [1f + rip]",
        "push {tmp}",
        "retfq",
        "1:",
        sel = in(reg) u64::from(sel.0),
        tmp = lateout(reg) _,
    );
}

pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

pub fn htl_indefinite() -> ! {
    loop {
        hlt();
    }
}
