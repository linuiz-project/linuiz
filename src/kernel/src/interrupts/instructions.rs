use core::arch::asm;

/// Enables interrupts for the current core.
///
/// SAFETY: Enabling interrupts early can result in unexpected behaviour.
#[inline(always)]
pub unsafe fn enable() {
    #[cfg(target_arch = "x86_64")]
    asm!("sti", options(nostack, nomem));

    #[cfg(target_arch = "riscv64")]
    crate::arch::rv64::registers::sstatus::set_sie(true);
}

/// Disables interrupts for the current core.
///
/// SAFETY: Disabling interrupts can cause the system to become unresponsive if they are not re-enabled.
#[inline(always)]
pub unsafe fn disable() {
    #[cfg(target_arch = "x86_64")]
    asm!("cli", options(nostack, nomem));

    #[cfg(target_arch = "riscv64")]
    crate::arch::rv64::registers::sstatus::set_sie(false);
}

/// Returns whether or not interrupts are enabled for the current core.
#[inline(always)]
pub fn are_enabled() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::registers::RFlags::read().contains(crate::arch::x64::registers::RFlags::INTERRUPT_FLAG)
    }

    #[cfg(target_arch = "riscv64")]
    {
        crate::arch::rv64::registers::sstatus::get_sie()
    }
}

/// Disables interrupts, executes the given [`FnOnce`], and re-enables interrupts if they were prior.
pub fn without<R>(func: impl FnOnce() -> R) -> R {
    let interrupts_enabled = are_enabled();

    if interrupts_enabled {
        unsafe { disable() };
    }

    let return_value = func();

    if interrupts_enabled {
        unsafe { enable() };
    }

    return_value
}

/// Waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_for() {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        asm!("hlt", options(nostack, nomem, preserves_flags));

        #[cfg(target_arch = "riscv64")]
        asm!("wfi", options(nostack, nomem, preserves_flags));
    }
}

/// Indefinitely waits for the next interrupt on the current core.
#[inline(always)]
pub fn wait_loop() -> ! {
    loop {
        wait_for()
    }
}
