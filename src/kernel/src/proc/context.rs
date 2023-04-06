use crate::uptr;

#[cfg(target_arch = "x86_64")]
mod arch_context {

    pub struct Registers {
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
}

use arch_context::*;

pub struct Context {
    state: State,
    registers: arch_context::Registers,
}

pub struct State {
    pub ip: uptr,
    pub sp: uptr,
}
