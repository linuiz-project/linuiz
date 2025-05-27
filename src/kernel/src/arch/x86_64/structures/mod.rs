pub mod gdt;
pub mod idt;
pub mod ioapic;
// pub mod tss;

/// A struct describing a pointer to a descriptor table (GDT / IDT).
/// This is in a format suitable for giving to 'lgdt' or 'lidt'.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed(2))]
pub struct DescriptorTablePointer {
    /// Size of the DT in bytes, less 1.
    pub limit: u16,

    /// Memory offset (pointer) to the table.
    pub base: u64,
}
