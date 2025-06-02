bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    struct SegmentDescriptor: u64 {
        /// Set by the processor if this segment has been accessed. Only cleared by software.
        /// *Setting* this bit in software prevents GDT writes on first use.
        const ACCESSED = 1 << 40;
        /// For 32-bit data segments, sets the segment as writable. For 32-bit code segments,
        /// sets the segment as _readable_. In 64-bit mode, ignored for all segments.
        const WRITABLE = 1 << 41;
        /// For code segments, sets the segment as “conforming”, influencing the
        /// privilege checks that occur on control transfers. For 32-bit data segments,
        /// sets the segment as "expand down". In 64-bit mode, ignored for data segments.
        const CONFORMING = 1 << 42;
        /// This flag must be set for code segments and unset for data segments.
        const EXECUTABLE = 1 << 43;
        /// This flag must be set for user segments (in contrast to system segments).
        const USER_SEGMENT = 1 << 44;
        /// These two bits encode the Descriptor Privilege Level (DPL) for this descriptor.
        /// If both bits are set, the DPL is Ring 3, if both are unset, the DPL is Ring 0.
        const DPL_RING_3 = 3 << 45;
        /// Must be set for any segment, causes a segment not present exception if not set.
        const PRESENT = 1 << 47;
        /// Available for use by the Operating System
        const AVAILABLE = 1 << 52;
        /// Must be set for 64-bit code segments, unset otherwise.
        const LONG_MODE = 1 << 53;
        /// Use 32-bit (as opposed to 16-bit) operands. If [`LONG_MODE`][Self::LONG_MODE] is set,
        /// this must be unset. In 64-bit mode, ignored for data segments.
        const DEFAULT_SIZE = 1 << 54;
        /// Limit field is scaled by 4096 bytes. In 64-bit mode, ignored for all segments.
        const GRANULARITY = 1 << 55;
        /// Bits `0..=15` of the limit field (ignored in 64-bit mode)
        const LIMIT_0_15 = 0xFFFF;
        /// Bits `16..=19` of the limit field (ignored in 64-bit mode)
        const LIMIT_16_19 = 0xF << 48;
        /// Bits `0..=23` of the base field (ignored in 64-bit mode, except for fs and gs)
        const BASE_0_23 = 0xFF_FFFF << 16;
        /// Bits `24..=31` of the base field (ignored in 64-bit mode, except for fs and gs)
        const BASE_24_31 = 0xFF << 56;

        /// Common bits shared in most kinds of segment descriptors.
        const COMMON =
            Self::USER_SEGMENT.bits()
            | Self::PRESENT.bits()
            | Self::WRITABLE.bits()
            | Self::ACCESSED.bits()
            | Self::LIMIT_0_15.bits()
            | Self::LIMIT_16_19.bits()
            | Self::GRANULARITY.bits();

        const KCODE_SEGMENT =
            Self::COMMON.bits()
            | Self::EXECUTABLE.bits()
            | Self::LONG_MODE.bits();

        const KDATA_SEGMENT =
            Self::COMMON.bits()
            | Self::DEFAULT_SIZE.bits();

        const UDATA_SEGMENT =
            Self::COMMON.bits()
            | Self::DEFAULT_SIZE.bits()
            | Self::DPL_RING_3.bits();

        const UCODE_SEGMENT =
            Self::COMMON.bits()
            | Self::EXECUTABLE.bits()
            | Self::LONG_MODE.bits()
            | Self::DPL_RING_3.bits();
    }
}

/// The GDT layout is very specific, due to the behaviour of the **IA32_STAR** MSR and its
/// affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
/// standard set by the aforementioned **IA32_STAR** MSR. Details can be found in the description of
/// the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
///
/// Additionally, x86 requires that the first GDT entry be null (i.e. no segment information).
static GDT: [SegmentDescriptor; 5] = [
    SegmentDescriptor::empty(),
    SegmentDescriptor::KCODE_SEGMENT,
    SegmentDescriptor::KDATA_SEGMENT,
    SegmentDescriptor::UDATA_SEGMENT,
    SegmentDescriptor::UCODE_SEGMENT,
];

pub const KCODE_SELECTOR: SegmentSelector = SegmentSelector::new(1, PrivilegeLevel::Ring0).unwrap();
pub const KDATA_SELECTOR: SegmentSelector = SegmentSelector::new(2, PrivilegeLevel::Ring0).unwrap();
pub const UDATA_SELECTOR: SegmentSelector = SegmentSelector::new(3, PrivilegeLevel::Ring3).unwrap();
pub const UCODE_SELECTOR: SegmentSelector = SegmentSelector::new(4, PrivilegeLevel::Ring3).unwrap();

/// ## Safety
///
/// This function should be executed only once, and prior to any point when the FS/GS _BASE MSRs will
/// be in use, as they are cleared when this function is run.
pub unsafe fn load() {
    use core::arch::asm;

    let gdt_dtptr = crate::arch::x86_64::structures::DescriptorTablePointer {
        limit: u16::try_from(core::mem::size_of_val(&GDT) - 1).unwrap(),
        base: GDT.as_ptr().addr().try_into().unwrap(),
    };

    // Safety: The GDT is properly formed, and the descriptor table pointer is
    //         set to the GDT's memory location, with the requisite limit set
    //         correctly (size in bytes, less 1).
    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) &gdt_dtptr,
            options(readonly, nostack, preserves_flags)
        );
    }

    // Safety: This is special since we cannot directly move to CS; x86 requires the instruction
    //         pointer and CS to be set at the same time. To do this, we push the new segment selector
    //         and return value onto the stack and use a "far return" (`retfq`) to reload CS and
    //         continue at the end of our function.
    //
    //         Note we cannot use a "far call" (`lcall`) or "far jmp" (`ljmp`) to do this because then we
    //         would only be able to jump to 32-bit instruction pointers. Only Intel implements support
    //         for 64-bit far calls/jumps in long-mode, AMD does not.
    unsafe {
        asm!(
            "push {sel}",
            "lea {tmp}, [55f + rip]",
            "push {tmp}",
            "retfq",
            "55:",
            sel = in(reg) u64::from(KCODE_SELECTOR.as_u16()),
            tmp = lateout(reg) _,
            options(preserves_flags),
        );
    }

    // Safety: While setting the ES & DS segment registers to null is perfectly safe, setting
    //         the FS & GS segment registers (on Intel only, not AMD) clears the respective
    //         FS/GS base. Thus, it is imperative that this function not be run after the GS
    //         base has been loaded with the CPU thread-local state structure pointer.
    unsafe {
        // Because this is x86, everything is complicated. It's important we load the extra
        // data segment registers (FS/GS) with the null descriptors, because if they don't
        // point to a null descriptor, then when CPL changes, the processor will clear the
        // base and limit of the relevant descriptor.
        //
        // This has the fun behavioural side-effect of ALSO clearing the FS/GS _BASE MSRs,
        // thus making any code involved in the CPL change context unable to access thread-local or
        // process-local state (when those MSRs are in use for the purpose).
        asm!(
            "
            mov ss, {sel:x}

            push rax        # store `rax`
            xor rax, rax    # zero-out `rax`
            # zero-out segment registers
            mov es, ax
            mov ds, ax
            mov fs, ax
            mov gs, ax
            pop rax         # restore `rax`
            ",
            sel = in(reg) KDATA_SELECTOR.as_u16(),
            options(preserves_flags)
        );
    }
}

/// Specifies which element to load into a segment from
/// descriptor tables (i.e., is a index to LDT or GDT table
/// with some additional flags).
///
/// See Intel 3a, Section 3.4.2 "Segment Selectors"
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentSelector(u16);

impl SegmentSelector {
    /// Can be used as a selector into a non-existent segment and assigned to segment registers,
    /// e.g. data segment register in ring 0
    pub const NULL: Self = Self(0);

    /// Creates a new [`SegmentSelector`]
    pub const fn new(index: u16, rpl: PrivilegeLevel) -> Option<SegmentSelector> {
        match (index, rpl) {
            (1 | 2, PrivilegeLevel::Ring0) | (3 | 4, PrivilegeLevel::Ring3) => {
                Some(SegmentSelector(index << 3 | (rpl as u16)))
            }

            (_, _) => None,
        }
    }

    /// Returns the selector as a raw u16.
    pub fn as_u16(self) -> u16 {
        self.0
    }

    /// Returns the GDT index.
    pub fn gdt_index(self) -> u16 {
        self.0 >> 3
    }

    /// Returns the requested privilege level.
    #[inline]
    pub fn rpl(self) -> PrivilegeLevel {
        PrivilegeLevel::from_u16(self.0 & 0b11)
    }
}

impl core::fmt::Debug for SegmentSelector {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SegmentSelector").field("rpl", &self.rpl()).field("gdt_index", &self.gdt_index()).finish()
    }
}

/// Represents a protection ring level.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PrivilegeLevel {
    /// Privilege-level 0 (most privilege): This level is used by critical system-software
    /// components that require direct access to, and control over, all processor and system
    /// resources. This can include BIOS, memory-management functions, and interrupt handlers.
    Ring0 = 0,

    /// Privilege-level 1 (moderate privilege): This level is used by less-critical system-
    /// software services that can access and control a limited scope of processor and system
    /// resources. Software running at these privilege levels might include some device drivers
    /// and library routines. The actual privileges of this level are defined by the
    /// operating system.
    Ring1 = 1,

    /// Privilege-level 2 (moderate privilege): Like level 1, this level is used by
    /// less-critical system-software services that can access and control a limited scope of
    /// processor and system resources. The actual privileges of this level are defined by the
    /// operating system.
    Ring2 = 2,

    /// Privilege-level 3 (least privilege): This level is used by application software.
    /// Software running at privilege-level 3 is normally prevented from directly accessing
    /// most processor and system resources. Instead, applications request access to the
    /// protected processor and system resources by calling more-privileged service routines
    /// to perform the accesses.
    Ring3 = 3,
}

impl PrivilegeLevel {
    /// Creates a `PrivilegeLevel` from a numeric value. The value must be in the range 0..4.
    ///
    /// This function panics if the passed value is >3.
    #[inline]
    pub const fn from_u16(value: u16) -> PrivilegeLevel {
        match value {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("invalid privilege level"),
        }
    }
}
