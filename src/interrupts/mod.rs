use crate::{
    cpu::{context::Context, local_state::LocalState},
    time::LocalTimer,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

pub mod exceptions;
pub mod syscall;

#[repr(u8)]
#[derive(Debug, TryFromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum Vector {
    Watchdog = 0x20,
    Timer = 0x21,
    Error = 0x22,
    PerformanceCounter = 0x23,
    ThermalSensor = 0x24,
    CMCI = 0x25,
    External = 0x26,

    Syscall = 0x80,

    Spurious = 0xFF,
}

/// Enables interrupts for the current processor.
pub fn enable() {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::instructions::__sti();
        }

        _ => { unimplemented!() }
    }
}

/// Disables interrupts for the current processor.
pub fn disable() {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::instructions::__cli();
        }

        _ => { unimplemented!() }
    }
}

/// Whether or not interrupts are enabled for the current processor.
pub fn is_enabled() -> bool {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::registers::ProcessorFlags::read()
                .contains(crate::arch::x86_64::registers::ProcessorFlags::INTERRUPT_FLAG)
        }

        _ => { unimplemented!() }
    }
}

/// Waits for the next interrupt on the current processor.
pub fn wait_next() {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::instructions::__hlt();
        }

        _ => { unimplemented!() }
    }
}

/// Disables interrupts if they were enabled, executes `func`, then re-enables
/// interrupts if they were disabled.
#[inline]
pub fn uninterruptable<T>(func: impl FnOnce() -> T) -> T {
    cfg_select! {
        test => { func() }

        not(test) => {
            let interrupts_enabled = is_enabled();

            if interrupts_enabled {
                disable();
            }

            let return_value = func();

            if interrupts_enabled {
                enable();
            }

            return_value
        }
    }
}

/// Indefinitely waits for the next interrupt on the current processor.
pub fn wait_indefinite() -> ! {
    loop {
        wait_next();
    }
}

pub fn handle(vector: Vector, context: &mut Context) {
    match vector {
        Vector::Timer => {
            LocalState::with_scheduler(|scheduler| {
                scheduler.interrupt_task(context);
            });

            LocalState::with_timer(LocalTimer::set_preemption_wait);
        }

        Vector::Syscall => {
            let result = crate::interrupts::syscall::handle(
                context.registers().syscall_vector(),
                context.registers().syscall_arg1(),
                context.registers().syscall_arg2(),
                context.registers().syscall_arg3(),
                context.registers().syscall_arg4(),
                context,
            );

            trace!("{result:#X?}");

            context.registers_mut().set_syscall_result(result);
        }

        vector => unimplemented!("unhandled interrupt vector: {vector:?}"),
    }
}
