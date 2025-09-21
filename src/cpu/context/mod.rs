use core::num::NonZero;

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[derive(Debug, Clone)]
pub struct Context {
    execution: Execution,
    registers: Registers,
}

impl Context {
    pub const fn new(execution: Execution, registers: Registers) -> Self {
        Self {
            execution,
            registers,
        }
    }

    /// A [`Context`] that uses [`crate::interrupts::wait_indefinite`] as the
    /// function pointer, and is never user-level.
    pub fn new_idle(stack_address: NonZero<usize>) -> Self {
        fn idle() {
            crate::interrupts::wait_indefinite();
        }

        Self::new_from_fn(idle, stack_address, false)
    }

    pub fn new_from_fn(func: fn(), stack_address: NonZero<usize>, is_user: bool) -> Self {
        #[allow(clippy::as_conversions)]
        let execution = if is_user {
            Execution::new_user(func as usize, stack_address.get())
        } else {
            Execution::new_kernel(func as usize, stack_address.get())
        };

        Self {
            execution,
            registers: Registers::default(),
        }
    }

    pub fn new_from_fn_with_arg(
        func: extern "sysv64" fn(usize),
        arg: usize,
        stack_address: NonZero<usize>,
        is_user: bool,
    ) -> Self {
        let registers = cfg_select! {
            target_arch = "x86_64" => {
                Registers {
                    rdi: arg,
                    ..Default::default()
                }
            }

            _ => { unimplemented!() }
        };

        let execution = if is_user {
            #[allow(clippy::as_conversions)]
            Execution::new_user(func as usize, stack_address.get())
        } else {
            #[allow(clippy::as_conversions)]
            Execution::new_kernel(func as usize, stack_address.get())
        };

        Self {
            execution,
            registers,
        }
    }

    pub fn execution(&self) -> &Execution {
        &self.execution
    }

    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }
}
