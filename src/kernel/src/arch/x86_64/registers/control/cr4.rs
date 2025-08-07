//! Facilities for managing the bits in the `CR4` register.
//!
//! Notably, [`CR4`] uses simple [`CR4::read`] and [`CR4::write`] methods,
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
        /// **Virtual-8086 Mode Extensions**
        ///
        /// Enables interrupt and exception handling extensions in virtual-8086
        /// mode when set; disables the extensions when clear. Use of the
        /// virtual mode extensions can improve the performance of virtual-8086
        /// applications by eliminating the overhead of calling the virtual-8086
        /// monitor to handle interrupts and exceptions that occur while
        /// executing an 8086 program, and instead redirecting the interrupts
        /// and exceptions back to the 8086 program’s handlers. It also provides
        /// hardware support for a virtual interrupt flag (`VIF`) to improve
        /// reliability of running 8086 programs in multi-tasking and
        /// multiple-processor environments.
        const VME = 1 << 0;

        /// **Protected-Mode Virtual Interrupts**
        ///
        /// Enables hardware support for a virtual interrupt flag (`VIF`) in
        // protected mode when set; disables the `VIF` flag in protected mode
        /// when clear.
        const PVI = 1 << 1;

        /// **Time Stamp Disable**
        ///
        /// Restricts the execution of the `RDTSC` instruction to procedures
        /// running at privilege level 0 when set; allows `RDTSC` instruction to
        /// be executed at any privilege level when clear. This bit also applies
        /// to the `RDTSCP` instruction if supported (if
        /// `CPUID.80000001H:EDX[27]` is `1`).
        const TSD = 1 << 2;

        /// **Debugging Extensions**
        ///
        /// References to debug registers `DR4` and `DR5` cause an undefined
        /// opcode (`#UD`) exception to be generated when set; when clear,
        /// processor aliases references to registers `DR4` and `DR5` for
        /// compatibility with software written to run on earlier IA-32
        /// processors.
        const DE = 1 << 3;

        /// **Page Size Extensions**
        ///
        /// Enables 4-MByte pages with 32-bit paging when set; restricts 32-bit
        /// paging to pages of 4 KBytes when clear.
        const PSE = 1 << 4;

        /// **Physical Address Extension**
        ///
        /// When set, enables paging to produce physical addresses with more
        /// than 32 bits. When clear, restricts physical addresses to 32 bits.
        ///
        /// # Remarks
        ///
        /// - [`PAE`] must be set before entering IA-32e mode.
        const PAE = 1 << 5;

        /// **Machine-Check Enable**
        ///
        /// Enables the machine-check exception when set; disables the
        /// machine-check exception when clear.
        const MCE = 1 << 6;

        /// **Page Global Enable**
        ///
        /// Enables the global page feature when set; disables the global page
        /// feature when clear. The global page feature allows frequently used
        /// or shared pages to be marked as global to all users (done with the
        /// global flag, bit 8 in a page-table entry). Global pages are not
        /// flushed from the translation-lookaside buffer (TLB) on a task switch
        /// or a write to register [`CR3`][crate::arch::x86_64::registers::control::CR3].
        ///
        /// # Remarks
        ///
        /// - Introduced in the P6 family processors.
        /// - When enabling the global page feature, paging must be enabled (by
        ///   setting the `PG` flag in control register
        ///   [`CR0`][crate::arch::x86_64::registers::control::CR0]) before the
        ///   [`PGE`] flag is set. Reversing this sequence may affect program
        ///   correctness, and processor performance will be impacted.
        const PGE = 1 << 7;

        /// **Performance-Monitoring Counter Enable**
        ///
        /// Enables execution of the `RDPMC` instruction for programs or
        /// procedures running at any protection level when set; `RDPMC`
        /// instruction can be executed only at protection level 0 when clear.
        const PCE = 1 << 8;

        /// **Operating System Support for FXSAVE and FXRSTOR instructions**
        ///
        /// When set, this flag:
        /// 1. Indicates to software that the operating system supports the use
        ///    of the `FXSAVE` and `FXRSTOR` instructions.
        /// 2. Enables the `FXSAVE` and `FXRSTOR` instructions to save and
        ///    restore the contents of the XMM and MXCSR registers along with
        ///    the contents of the x87 FPU and MMX registers.
        /// 3. Enables the processor to execute SSE/SSE2/SSE3/SSSE3/SSE4
        ///    instructions, with the exception of the `PAUSE`, `PREFETCH`,
        ///    `SFENCE`, `LFENCE`, `MFENCE`, `MOVNTI`, `CLFLUSH`, `CRC32`, and
        ///    `POPCNT`.
        ///
        /// If this flag is clear, the `FXSAVE` and `FXRSTOR` instructions will save and restore the contents of the x87 FPU and MMX registers, but they may not save and restore the contents of the XMM and MXCSR registers.
        ///
        /// # Remarks
        ///
        /// - The processor will generate an invalid opcode exception (`#UD`) if
        ///   it attempts to execute any SSE/SSE2/SSE3 instruction, with the
        ///   exception of `PAUSE`, `PREFETCH`, `SFENCE`, `LFENCE`, `MFENCE`,
        ///   `MOVNTI`, `CLFLUSH`, `CRC32`, and `POPCNT`.
        /// - `CPUID` feature flag `FXSR` indicates availability of the
        ///   `FXSAVE`/`FXRSTOR` instructions. The `OSFXSR` bit provides
        ///   operating system software with a means of enabling
        ///   `FXSAVE`/`FXRSTOR` to save/restore the contents of the X87 FPU,
        ///   XMM, and MXCSR registers. Consequently `OSFXSR` bit indicates that
        ///   the operating system provides context switch support for
        ///   SSE/SSE2/SSE3/SSSE3/SSE4.
        const OSFXSR = 1 << 9;

        /// **Operating System Support for Unmasked SIMD Floating-Point Exceptions**
        ///
        /// When set, indicates that the operating system supports the handling
        /// of unmasked SIMD floating-point exceptions through an exception
        /// handler that is invoked when a SIMD floating-point exception (`#XM`)
        /// is generated. SIMD floating-point exceptions are only generated by
        /// SSE/SSE2/SSE3/SSE4.1 SIMD floating-point instructions.
        ///
        /// # Remarks
        ///
        /// If this flag is not set, the processor will generate an invalid
        /// opcode exception (`#UD`) whenever it detects an unmasked SIMD
        /// floating-point exception.
        const OSXMMEXCPT = 1 << 10;

        /// **User-Mode Instruction Prevention**
        ///
        /// When set, the `SGDT`, `SIDT`, `SLDT`, `SMSW`, and `STR` instructions
        /// cannot be executed if the current privilege level is less than 0, or
        /// the processor will raise a causes a general-protection exception
        /// (`#GP`).
        const UMIP = 1 << 11;

        /// **57-bit Linear Addresses**
        ///
        /// When set in IA-32e mode, the processor uses 5-level paging to
        /// translate 57-bit linear addresses. When clear in IA-32e mode, the
        /// processor uses 4-level paging to translate 48-bit linear addresses.
        ///
        /// # Remarks
        ///
        /// This bit cannot be modified in IA-32e mode.
        const LA57 = 1 << 12;

        /// **VMX-Enable**
        ///
        /// Enables VMX operation when set.
        ///
        /// *See: Chapter 25, “Introduction to Virtual Machine Extensions” in
        /// Volume 3 of the [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const VMXE = 1 << 13;

        /// **SMX-Enable**
        ///
        /// Enables SMX operation when set.
        ///
        /// *See: Chapter 7, "Safer Mode Extensions Reference" in Volume 2 of
        /// the [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const SMXE = 1 << 14;

        /// **FSGSBASE-Enable**
        ///
        /// Enables the instructions `RDFSBASE`, `RDGSBASE`, `WRFSBASE`, and
        /// `WRGSBASE`.
        const FSGSBASE = 1 << 16;

        /// **PCID-Enable**
        ///
        /// Enables process-context identifiers (PCIDs) when set.
        ///
        /// *See: Section 5.10.1, “Process Context Identifiers (PCIDs)" in
        /// Volume 3 of the [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const PCIDE = 1 << 17;

        /// **XSAVE and Processor Extended States-Enable**
        ///
        /// When set, this flag:
        /// 1. Indicates (via `CPUID.01H:ECX.OSXSAVE[bit 27]`) that the
        ///    operating system supports the use of the `XGETBV`, `XSAVE`, and
        ///    `XRSTOR` instructions by general software.
        /// 2. Enables the `XSAVE` and `XRSTOR` instructions to save and restore
        ///    the x87 FPU state (including MMX registers), the SSE state (XMM
        ///    registers and MXCSR), along with other processor extended states
        ///    enabled in XCR0.
        /// 3. Enables the processor to execute `XGETBV` and `XSETBV`
        ///    instructions in order to read and write `XCR0`.
        ///
        /// *See: Section 2.6 of Volume 3 and Chapter 15, "System  Programming
        /// for Instruction Set Extensions and Processor Extended States" in
        /// Volume 3 of the [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const OSXSAVE = 1 << 18;

        /// **Key-Locker-Enable**
        ///
        /// When set, the `LOADIWKEY` instruction is enabled; in addition, if
        /// support for the AES Key Locker instructions has been activated by
        /// system firmware, `CPUID.19H:EBX.AESKLE[bit 0]` is enumerated as `1`
        /// and the AES Key Locker instructions are enabled.*
        ///
        /// When clear, `CPUID.19H:EBX.AESKLE[bit 0]` is enumerated as `0` and
        /// execution of any Key Locker instruction causes an invalid-opcode
        /// exception (`#UD`).
        ///
        /// \*: Software can check `CPUID.19H:EBX.AESKLE[bit 0]` after setting
        /// [`KL`] to determine whether the AES Key Locker instructions have
        /// been enabled.
        ///
        /// # Remarks
        ///
        /// - Some processors may allow enabling of those instructions without
        ///   activation by system firmware.
        /// - Some processors may not support use of the AES Key Locker
        ///   instructions in system-management mode (SMM). Those processors
        ///   enumerate `CPUID.19H:EBX.AESKLE[bit 0]` as `0` in SMM regardless
        ///   of the setting of [`KL`].
        const KL = 1 << 19;

        /// **Supervisor Mode Execution Prevention**
        ///
        /// Enables supervisor-mode execution prevention (SMEP) when set.
        ///
        /// *See: Section 5.6, "Access Rights" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const SMEP = 1 << 20;

        /// **Supervisor Mode Access Prevention**
        ///
        /// Enables supervisor-mode access prevention (SMAP) when set.
        ///
        /// *See: Section 5.6, "Access Rights" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const SMAP = 1 << 21;


        /// **Protection Key Enable for user-mode pages**
        ///
        /// 4-level paging and 5-level paging associate each user-mode linear
        /// address with a protection key. When set, this flag indicates (via
        /// `CPUID.(EAX=07H,ECX=0H):ECX.OSPKE [bit 4]`) that the operating
        /// system supports use of the `PKRU` register to specify—for each
        /// protection key—whether user-mode linear addresses with that
        /// protection key can be read or written.
        ///
        /// This bit also enables access to the `PKRU` register using the
        /// `RDPKRU` and `WRPKRU` instructions.
        const PKE = 1 << 22;

        /// **Control-flow Enforcement Technology**
        ///
        /// Enables control-flow enforcement technology when set.
        ///
        /// *See: Chapter 4, "Control-flow Enforcement Technology (CET)" in
        /// Volume 1 of the [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const CET = 1 << 23;

        /// **Protection Keys for Supervisor-mode pages**
        ///
        /// 4-level paging and 5-level paging associate each supervisor-mode
        /// linear address with a protection key. When set, this flag allows use
        /// of the `IA32_PKRS` model-specific register to specify—for each
        /// protection key-whether supervisor-mode linear addresses with that
        /// protection key can be read or written.
        const PKS = 1 << 24;

        /// **User Interrupts Enable**
        ///
        /// Enables user interrupts when set, including user-interrupt delivery,
        /// user-interrupt notification identification, and the user-interrupt
        /// instructions.
        const UINTR = 1 << 25;

        /// **Supervisor LAM enable**
        ///
        /// When set, enables LAM (linear-address masking) for supervisor
        /// pointers.
        ///
        /// *See: Section 4.4, "Linear Address Masking" in Volume 3 of the
        /// [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).*
        const LAM_SUP = 1 << 28;
    }
}

pub struct CR4;

impl CR4 {
    pub fn read() -> Flags {
        let value: usize;

        // Safety: Reading a control register is pure.
        unsafe {
            core::arch::asm!(
                "mov {}, cr4",
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
                "mov cr4, {}",
                in(reg) flags.bits()
            );
        }
    }

    /// # Safety
    ///
    /// - Changing the currently active flags must not cause a processor
    ///   exception.
    pub unsafe fn enable(flags: Flags) {
        let mut current_flags = CR4::read();
        trace!("Current `CR4`: {current_flags:?}");

        trace!("Enabling `CR4`: {flags:?}");
        current_flags.insert(flags);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR4::write(current_flags);
        }
    }

    /// # Safety
    ///
    /// - Changing the currently active flags must not cause a processor
    ///   exception.
    pub unsafe fn disable(flags: Flags) {
        let mut current_flags = CR4::read();
        trace!("Current `CR4`: {current_flags:?}");

        trace!("Disabling `CR4`: {flags:?}");
        current_flags.remove(flags);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR4::write(current_flags);
        }
    }
}
