//! Facilities for managing the bits in the `CR0` register.
//!
//! Notably, [`CR0`] uses simple [`CR0::read`] and [`CR0::write`] methods,
//! rather than having discrete methods for modifying each bit. This latter
//! architecture was tried, but proved to present challenges (such as
//! pretty-printing out all of the meaningful bits in the register).
//!
//! Additionally, the discrete method approach offers next to not benefits
//! (except perhaps erroring inside the discrete methods, very obviously
//! indicating which feature enablement caused the error).

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: usize {
        /// **Protection Enable**
        ///
        /// Enables protected mode when set; enables real-address mode when
        /// clear. This flag does not enable paging directly, only enabling
        /// segment-level protection. To enable paging, both the `PE` and `PG`
        /// flags must be set.
        ///
        /// *See also: Section 11.9, "Mode Switching" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const PE = 1 << 0;

        /// **Monitor Coprocessor**
        ///
        /// Controls the interaction of the `WAIT`/`FWAIT` instruction with the
        /// `TS` flag (bit 3 of `CR0`).
        ///
        /// **Recommended setting of this flag:**
        ///
        /// | `CR0.EM` | `CR0.MP` | `CR0.NE` |                                                                                 IA-32 Processor                                                                                |
        /// |:--------:|:--------:|:--------:|:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------:|
        /// |     1    |     0    |     1    |                                    Intel486™ SX, Intel386™ DX, and Intel386™ SX processors only, without the presence of a math coprocessor.                                   |
        /// |     0    |     1    |  0 or 1* | Pentium 4, Intel Xeon, P6 family, Pentium, Intel486™ DX, and Intel 487 SX processors, and Intel386 DX and Intel386 SX processors when a companion math coprocessor is present. |
        /// |     0    |     1    |  0 or 1* |                                                                    More recent Intel 64 or IA-32 processors.                                                                   |
        ///
        /// **Iteractions of the `EM`, `MP`, and `TS` flags:**
        ///
        /// | `CR4.OSFXSR` | `CR4.OSXMMEXCPT` | SSE, SSE2, SSE3, SSE4_1 | `CR0.EM` | `CR0.MP` | `CR0.TS` |                                           Action                                           |
        /// |:------------:|:----------------:|:-----------------------:|:--------:|:--------:|:--------:|:------------------------------------------------------------------------------------------:|
        /// |       0      |         X        |            X            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            0            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            1            |     1    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         0        |            1            |     0    |     1    |     0    | Execute instruction; `#UD` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         1        |            1            |     0    |     1    |     0    | Execute instruction; `#XM` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         X        |            1            |     0    |     1    |     1    |                                       `#NM` Exception                                      |
        const MP = 1 << 1;

        /// **Emulation**
        ///
        /// Indicates that the processor does not have an internal or external
        /// x87 FPU when set; indicates an x87 FPU is present when clear. This
        /// flag also affects the execution of MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        /// instructions.
        ///
        /// **Recommended setting of this flag:**
        ///
        /// | `CR0.EM` | `CR0.MP` | `CR0.NE` |                                                                                 IA-32 Processor                                                                                |
        /// |:--------:|:--------:|:--------:|:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------:|
        /// |     1    |     0    |     1    |                                    Intel486™ SX, Intel386™ DX, and Intel386™ SX processors only, without the presence of a math coprocessor.                                   |
        /// |     0    |     1    |  0 or 1* | Pentium 4, Intel Xeon, P6 family, Pentium, Intel486™ DX, and Intel 487 SX processors, and Intel386 DX and Intel386 SX processors when a companion math coprocessor is present. |
        /// |     0    |     1    |  0 or 1* |                                                                    More recent Intel 64 or IA-32 processors.                                                                   |
        ///
        /// **Iteractions of the `EM`, `MP`, and `TS` flags:**
        ///
        /// | `CR4.OSFXSR` | `CR4.OSXMMEXCPT` | SSE, SSE2, SSE3, SSE4_1 | `CR0.EM` | `CR0.MP` | `CR0.TS` |                                           Action                                           |
        /// |:------------:|:----------------:|:-----------------------:|:--------:|:--------:|:--------:|:------------------------------------------------------------------------------------------:|
        /// |       0      |         X        |            X            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            0            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            1            |     1    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         0        |            1            |     0    |     1    |     0    | Execute instruction; `#UD` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         1        |            1            |     0    |     1    |     0    | Execute instruction; `#XM` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         X        |            1            |     0    |     1    |     1    |                                       `#NM` Exception                                      |
        const EM = 1 << 2;

        /// **Task Switched**
        ///
        /// Allows the saving of the x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        /// context on a task switch to be delayed until an x87
        /// FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4 instruction is actually executed by
        /// the new task. The processor sets this flag on every task switch and
        /// tests it when executing x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        /// instructions.
        ///
        /// # Remarks
        ///
        /// - If the `TS` flag is set and the `EM` flag (bit 2 of `CR0`) is
        ///   clear, a device-not-available exception (`#NM`) is raised prior to
        ///   the execution of any x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        ///   instruction; with the exception of `PAUSE`, `PREFETCH`, `SFENCE`,
        ///   `LFENCE`, `MFENCE`, `MOVNTI`, `CLFLUSH`, `CRC32`, and `POPCNT`.
        ///   See the paragraph below for the special case of the `WAIT`/`FWAIT`
        ///   instructions.
        /// - If the `TS` flag is set and the `MP` flag (bit 1 of `CR0`) and
        ///   `EM` flag are clear, an `#NM` exception is not raised prior to the
        ///   execution of an x87 FPU `WAIT`/`FWAIT` instruction.
        /// - If the `EM` flag is set, the setting of the `TS` flag has no
        ///   effect on the execution of x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        ///   instructions.
        ///
        /// **Actions taken when the processor encounters an x87 FPU instruction:**
        ///
        /// | `CR0.EM` | `CR0.MP` | `CR0.TS` |  Floating-Point |  `WAIT`/`FWAIT` |
        /// |:--------:|:--------:|:--------:|:---------------:|:---------------:|
        /// |     0    |     0    |     0    |     Execute     |     Execute     |
        /// |     0    |     0    |     1    | `#NM` Exception |     Execute     |
        /// |     0    |     1    |     0    |     Execute     |     Execute     |
        /// |     1    |     0    |     0    | `#NM` Exception | `#NM` Exception |
        /// |     1    |     0    |     0    | `#NM` Exception |     Execute     |
        /// |     1    |     0    |     1    | `#NM` Exception |     Execute     |
        /// |     1    |     1    |     0    | `#NM` Exception |     Execute     |
        /// |     1    |     1    |     1    | `#NM` Exception | `#NM` Exception |
        ///
        /// **Actions taken when the processor encounters an MMX instruction:**
        ///
        /// | `CR0.EM` | `CR0.MP`* | `CR0.TS` |      Action     |
        /// |:--------:|:---------:|:--------:|:---------------:|
        /// |     0    |     1     |     0    |     Execute     |
        /// |     0    |     1     |     1    | `#NM` Exception |
        /// |     1    |     1     |     0    | `#UD` Exception |
        /// |     1    |     1     |     1    | `#UD` Exception |
        ///
        /// **Actions taken when the processor encounters an SSE/SSE2/SSE3/SSSE3/SSE4 instruction:**
        ///
        /// | `CR4.OSFXSR` | `CR4.OSXMMEXCPT` | SSE, SSE2, SSE3, SSE4_1 | `CR0.EM` | `CR0.MP` | `CR0.TS` |                                           Action                                           |
        /// |:------------:|:----------------:|:-----------------------:|:--------:|:--------:|:--------:|:------------------------------------------------------------------------------------------:|
        /// |       0      |         X        |            X            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            0            |     X    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         X        |            1            |     1    |     1    |     X    |                                       `#UD` Exception                                      |
        /// |       1      |         0        |            1            |     0    |     1    |     0    | Execute instruction; `#UD` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         1        |            1            |     0    |     1    |     0    | Execute instruction; `#XM` exception if unmasked SIMD floating point exception is detected |
        /// |       1      |         X        |            1            |     0    |     1    |     1    |                                       `#NM` Exception                                      |
        ///
        /// The processor does not automatically save the context of the x87
        /// FPU, XMM, and MXCSR registers on a task switch. Instead, it sets the
        /// `TS` flag, which causes the processor to raise an `#NM` exception
        /// whenever it encounters an x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4
        /// instruction in the instruction stream for the new task (with the
        /// exception of the instructions listed above).
        ///
        /// The fault handler for the `#NM` exception can then be used to clear
        /// the `TS` flag (with the `CLTS` instruction) and save the context of
        /// the x87 FPU, XMM, and MXCSR registers. If the task never encounters
        /// an x87 FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4 instruction, the x87
        /// FPU/MMX/SSE/SSE2/SSE3/SSSE3/SSE4 context is never saved.
        const TS = 1 << 3;

        /// **Extension Type**
        ///
        /// In the Intel386 and Intel486 processors, this flag indicates support
        /// of Intel 387 DX math coprocessor instructions when set.
        ///
        /// # Remarks
        ///
        /// - Reserved in the Pentium 4, Intel Xeon, P6 family, and Pentium
        ///   processors.
        /// - In the Pentium 4, Intel Xeon, and P6 family processors, this flag
        ///   is hardcoded to `1`.
        const ET = 1 << 4;

        /// **Numeric Error**
        ///
        /// Enables the native (internal) mechanism for reporting x87 FPU errors
        /// when set; enables the PC-style x87 FPU error reporting mechanism
        /// when clear.
        ///
        /// # Remarks
        ///
        /// When the `NE` flag is clear and the `IGNNE#` input is asserted, x87
        /// FPU errors are ignored. When the `NE` flag is clear and the `IGNNE#`
        /// input is deasserted, an unmasked x87 FPU error causes the processor
        /// to assert the `FERR#` pin to generate an external interrupt and to
        /// stop instruction execution immediately before executing the next
        /// waiting floating-point instruction or `WAIT`/`FWAIT` instruction.
        ///
        /// The `FERR#` pin is intended to drive an input to an external
        /// interrupt controller (the `FERR#` pin emulates the `ERROR#` pin of
        /// the Intel 287 and Intel 387 DX math coprocessors). The `NE` flag,
        /// `IGNNE#` pin, and `FERR#` pin are used with external logic to
        /// implement PC-style error reporting. Using `FERR#` and `IGNNE#` to
        /// handle floating-point exceptions is deprecated by modern operating
        /// systems; this non-native approach also limits newer processors to
        /// operate with one logical processor active.
        const NE = 1 << 5;

        /// **Write Protect**
        ///
        /// When set, inhibits supervisor-level procedures from writing into
        /// read-only pages; when clear, allows supervisor-level procedures to
        /// write into read-only pages (regardless of the `U`/`S` bit setting;
        /// see Section 5.1.3 and Section 5.6 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)).
        ///
        /// # Remarks
        ///
        /// - Facilitates implementation of the copy-on-write method of creating
        ///   a new process (forking) used by operating systems such as UNIX.
        /// - Must be set before software can set [`CET`][crate::arch::x86_64::registers:control::cr4::Flags::CET],
        ///   and it cannot be cleared as long as [`CET`][crate::arch::x86_64::registers:control::cr4::Flags::CET]
        ///   is `1`.
        const WP = 1 << 16;

        /// **Alignment Mask**
        ///
        /// Enables automatic alignment checking when set; disables alignment
        /// checking when clear.
        ///
        /// # Remarks
        ///
        /// Alignment checking is performed only when the `AM` flag is set, the
        /// `AC` flag in the `EFLAGS` register is set, the current privilege
        /// level is `3`, and the processor is operating in either protected or
        /// virtual-8086 mode.
        const AM = 1 << 18;

        /// **Not Write-through**
        ///
        /// When the `NW` and `CD` flags are clear, write-back (for Pentium 4,
        /// Intel Xeon, P6 family, and Pentium processors) or write-through
        /// (for Intel486 processors) is enabled for writes that hit the cache
        /// and invalidation cycles are enabled.
        ///
        /// *See: Table 13-5 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
        /// for detailed information the effect of the `NW` flag on caching for
        /// other settings of the `CD` and `NW` flags.*
        const NW = 1 << 29;

        /// **Cache Disable**
        ///
        /// When the `CD` and `NW` flags are clear, caching of memory locations
        /// for the whole of physical memory in the processor’s internal (and
        /// external) caches is enabled. When the `CD` flag is set, caching is
        /// restricted as described in Table 13-5 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).
        ///
        /// To prevent the processor from accessing and updating its caches, the
        /// `CD` flag must be set and the caches must be invalidated so that no
        /// cache hits can occur.
        ///
        /// *See also: Section 13.5.3, "Preventing Caching", and Section 13.5,
        /// "Cache Control" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const CD = 1 << 30;

        /// **Paging**
        ///
        /// Enables paging when set; disables paging when clear.
        ///
        /// *See: Chapter 5, "Paging" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        ///
        /// # Remarks
        ///
        /// - When paging is disabled, all linear addresses are treated as
        ///   physical addresses.
        /// - Has no effect if the `PE` flag (bit 0 of register `CR0`) is not
        ///   also set; setting the `PG` flag when the `PE` flag is clear causes
        ///   a general-protection exception (`#GP`).
        const PG = 1 << 31;
    }
}

pub struct CR0;

impl CR0 {
    pub fn read() -> Flags {
        let value: usize;

        // Safety: Reading CR0 has no side effects.
        unsafe {
            core::arch::asm!(
                "mov {}, cr0",
                out(reg) value,
                options(nostack, nomem, preserves_flags)
            );
        }

        Flags::from_bits_truncate(value)
    }

    /// # Safety
    ///
    /// - Changing the currently active flags must not cause a processor
    ///   exception.
    unsafe fn write(flags: Flags) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            core::arch::asm!(
                "mov cr0, {}",
                in(reg) flags.bits()
            );
        }
    }

    /// # Safety
    ///
    /// - Changing the currently active flags must not cause a processor
    ///   exception.
    pub unsafe fn enable(flags: Flags) {
        let mut current_flags = CR0::read();
        trace!("Current `CR0`: {current_flags:?}");

        trace!("Enabling `CR0`: {flags:?}");
        current_flags.insert(flags);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR0::write(current_flags);
        }
    }

    /// # Safety
    ///
    /// - Changing the currently active flags must not cause a processor
    ///   exception.
    pub unsafe fn disable(flags: Flags) {
        let mut current_flags = CR0::read();
        trace!("Current `CR0`: {current_flags:?}");

        trace!("Disabling `CR0`: {flags:?}");
        current_flags.remove(flags);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR0::write(current_flags);
        }
    }
}
