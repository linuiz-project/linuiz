bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct RFlags : usize {
        /// Set by hardware if the last arithmetic operation generated a carry out of the most-significant
        /// bit of the result.
        const CARRY_FLAG = 1 << 0                   | Self::REQ_RESERVED;
        /// Set by hardware if the last result has an even number of 1 bits (only for some operations).
        const PARITY_FLAG = 1 << 2                  | Self::REQ_RESERVED;
        /// Set by hardware if the last arithmetic operation generated a carry out of bit 3 of the result.
        const AUXILIARY_CARRY_FLAG = 1 << 4         | Self::REQ_RESERVED;
        /// Set by hardware if last arithmetic operation resulted in a zero value.
        const ZERO_FLAG = 1 << 6                    | Self::REQ_RESERVED;
        /// Set by hardware if the last arithmetic operation resulted in a negative value.
        const SIGN_FLAG = 1 << 7                    | Self::REQ_RESERVED;
        /// Enable single-step mode for debugging.
        const TRAP_FLAG = 1 << 8                    | Self::REQ_RESERVED;
        /// Enable interrupts.
        const INTERRUPT_FLAG = 1 << 9               | Self::REQ_RESERVED;
        /// Determines the order in which strings are processed.
        const DIRECTION_FLAG = 1 << 10              | Self::REQ_RESERVED;
        /// Set by hardware to indicate that the sign bit of the result of the last signed integer
        /// operation differs from the source operands.
        const OVERFLOW_FLAG = 1 << 11               | Self::REQ_RESERVED;
        /// The low bit of the I/O Privilege Level field.
        ///
        /// Specifies the privilege level required for executing the I/O address-space instructions.
        const IOPL_LOW = 1 << 12                    | Self::REQ_RESERVED;
        /// The high bit of the I/O Privilege Level field.
        ///
        /// Specifies the privelege level required for executing I/O address-space instructions.
        const IOPL_HIGH = 1 << 13                   | Self::REQ_RESERVED;
        /// Used by `iret` in hardware task switch mode to determine if current task is nested.
        const NESTED_TASK = 1 << 14                 | Self::REQ_RESERVED;
        /// Allows restarting an instruction following an instruction breakpoint.
        const RESUME_FLAG = 1 << 16                 | Self::REQ_RESERVED;
        /// Enable the virtual-8086 mode.
        const VIRTUAL_8086_MODE = 1 << 17           | Self::REQ_RESERVED;
        /// Enable automatic alignment-checking if the CR0.AM is set. Only works
        /// if CPL is 3.
        const ALIGNMENT_CHECK = 1 << 18             | Self::REQ_RESERVED;
        /// Virtual image of the INERRUPT_FLAG bit.
        ///
        /// Used when virtual-8086 mode extensions (CR4.VME) or protected-mode virtual
        /// interrupts (CR4.PVI) are activated.
        const VIRTUAL_INTERRUPT = 1 << 19           | Self::REQ_RESERVED;
        /// Indicates that an external, maskable interrupt is pending.
        ///
        /// Used when virtual-8086 mode extensions (CR4.VME) or protected-mode virtual
        /// interrupts (CR4.PVI) are activated.
        const VIRTUAL_INTERRUPT_PENDING = 1 << 20   | Self::REQ_RESERVED;
        /// Processor feature identification flag.
        ///
        /// If this flag is modifiable, the CPU supports CPUID.
        const ID = 1 << 21;
    }
}

impl RFlags {
    const REQ_RESERVED: usize = 1 << 1;

    #[inline]
    pub fn read() -> Self {
        Self::from_bits_truncate(Self::read_raw())
    }

    #[inline]
    fn read_raw() -> usize {
        let result: usize;

        // Safety: Instruction block has no side effects.
        unsafe {
            core::arch::asm!(
                "pushf",
                "pop {}",
                out(reg) result,
                options(pure, nomem, preserves_flags)
            );
        }

        result
    }

    /// ### Safety
    ///
    /// Providing an invalid (in general, or for the current context) set of flags in undefined behaviour.
    #[inline]
    pub unsafe fn set_flags(flags: Self, set: bool) {
        let mut old_flags = Self::read();
        old_flags.set(flags, set);

        core::arch::asm!(
            "push {}",
            "popf",
            in(reg) old_flags.bits(),
            options(nostack, nomem, preserves_flags)
        );
    }
}
