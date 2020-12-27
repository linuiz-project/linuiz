use crate::structures::DescriptorTablePointer;

pub unsafe fn lidt(idt: &DescriptorTablePointer) {
    asm!("lidt [{}]", in(reg) idt, options(nostack));
}

pub fn breakpoint() {
    unsafe {
        asm!("int3");
    }
}

pub fn enable() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

pub fn disable() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}
