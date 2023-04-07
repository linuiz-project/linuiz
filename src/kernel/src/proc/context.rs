use crate::uptr;

#[cfg(target_arch = "x86_64")]
mod arch_context {
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct General {
        pub rax: u64,
        pub rbx: u64,
        pub rcx: u64,
        pub rdx: u64,
        pub rsi: u64,
        pub rdi: u64,
        pub rbp: u64,
        pub r8: u64,
        pub r9: u64,
        pub r10: u64,
        pub r11: u64,
        pub r12: u64,
        pub r13: u64,
        pub r14: u64,
        pub r15: u64,
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct Segment {
        pub cs: u16,
        pub ss: u16,
    }

    pub type Registers = (General, Segment);
}

pub use arch_context::*;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub ip: uptr,
    pub sp: uptr,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Context(State, Registers);

impl Context {
    pub fn new(state: State, regs: Registers) -> Self {
        Self(state, regs)
    }

    pub fn state(&self) -> &State {
        &self.0
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.0
    }

    pub fn regs(&self) -> &Registers {
        &self.1
    }

    pub fn regs_mut(&mut self) -> &mut Registers {
        &mut self.1
    }
}
