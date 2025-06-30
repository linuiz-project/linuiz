use bit_field::BitField;

bitflags! {
    /// Describes an page fault error code.
    ///
    /// This structure is defined by the following manual sections:
    ///   * AMD Volume 2: 8.4.2
    ///   * Intel Volume 3A: 4.7
    #[repr(transparent)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct PageFaultErrorCode: u64 {
        /// If this flag is set, the page fault was caused by a page-protection violation,
        /// else the page fault was caused by a not-present page.
        const PROTECTION_VIOLATION = 1;

        /// If this flag is set, the memory access that caused the page fault was a write.
        /// Else the access that caused the page fault is a memory read. This bit does not
        /// necessarily indicate the cause of the page fault was a read or write violation.
        const CAUSED_BY_WRITE = 1 << 1;

        /// If this flag is set, an access in user mode (CPL=3) caused the page fault. Else
        /// an access in supervisor mode (CPL=0, 1, or 2) caused the page fault. This bit
        /// does not necessarily indicate the cause of the page fault was a privilege violation.
        const USER_MODE = 1 << 2;

        /// If this flag is set, the page fault is a result of the processor reading a 1 from
        /// a reserved field within a page-translation-table entry.
        const MALFORMED_TABLE = 1 << 3;

        /// If this flag is set, it indicates that the access that caused the page fault was an
        /// instruction fetch.
        const INSTRUCTION_FETCH = 1 << 4;

        /// If this flag is set, it indicates that the page fault was caused by a protection key.
        const PROTECTION_KEY = 1 << 5;

        /// If this flag is set, it indicates that the page fault was caused by a shadow stack
        /// access.
        const SHADOW_STACK = 1 << 6;

        /// If this flag is set, it indicates that the page fault was caused by SGX access-control
        /// requirements (Intel-only).
        const SGX = 1 << 15;

        /// If this flag is set, it indicates that the page fault is a result of the processor
        /// encountering an RMP violation (AMD-only).
        const RMP = 1 << 31;
    }
}

/// An error code referencing a segment selector.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SelectorErrorCode(u64);

impl SelectorErrorCode {
    /// Create a [`SelectorErrorCode`]`. Returns [`None`]` if any reserved bits (16-64) are set.
    pub const fn new(flags: u64) -> Option<Self> {
        if (flags & 0xFFFF_FFFF_FFFF_0000) == 0 {
            Some(Self(flags))
        } else {
            None
        }
    }

    /// Indicates whether the exception occured during delivery of an
    /// external event, such as an interrupt, or earlier exception.
    pub fn is_external(self) -> bool {
        self.0.get_bit(0)
    }

    /// The descriptor table this error code refers to.
    pub fn table_kind(self) -> DescriptorTableKind {
        match self.0.get_bits(1..3) {
            0b00 => DescriptorTableKind::GDT,
            0b10 => DescriptorTableKind::LDT,
            0b01 | 0b11 => DescriptorTableKind::IDT,
            _ => unreachable!(),
        }
    }

    /// Table index of the entry which caused the error.
    pub fn table_index(self) -> u16 {
        self.0.get_bits(3..16).try_into().unwrap()
    }

    /// If true, the #SS or #GP has returned zero as opposed to a [`DescriptorTableErrorCode`].
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl core::fmt::Debug for SelectorErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut s = f.debug_struct("Selector Error");
        s.field("external", &self.is_external());
        s.field("descriptor table", &self.table_kind());
        s.field("index", &self.table_index());
        s.finish()
    }
}

/// The possible descriptor table values.
///
/// Used by the [`SelectorErrorCode`] to indicate which table caused the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorTableKind {
    /// Global Descriptor Table.
    GDT,
    /// Interrupt Descriptor Table.
    IDT,
    /// Logical Descriptor Table.
    LDT,
}
