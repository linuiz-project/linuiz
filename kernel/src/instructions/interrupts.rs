pub fn enable() {
    unsafe {
        asm!("sti", options(nostack, nomem));
    }
}

pub fn disable() {
    unsafe {
        asm!("cli", options(nostack, nomem));
    }
}

pub fn without_interrupts<F, R>(function: F) -> R
where
    F: FnOnce() -> R,
{
    let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();

    if interrupts_enabled {
        disable();
    }

    let return_value = function();

    if interrupts_enabled {
        enable();
    }

    return_value
}
