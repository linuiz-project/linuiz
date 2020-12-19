pub mod interrupts;
pub mod tables;

pub unsafe fn set_cs(sel: crate::structures::gdt::SegmentSelector) {
    asm!(
        "push {sel}",
        "lea {tmp}, [1F + rip]",
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
