pub mod gdt;
pub mod idt;
pub mod tss;

trait DescriptorTable {
    fn limit(&self) -> u16;
}

/// A struct describing a pointer to a descriptor table (GDT / IDT).
///
/// This is in a format suitable for giving to 'lgdt' or 'lidt'.
#[repr(C, packed(0x2))]
#[derive(Clone)]
pub struct DescriptorTablePointer {
    /// Size of the DT in bytes, less 1.
    limit: u16,

    /// Base address of the table.
    base: usize,
}

impl<T: DescriptorTable> From<&T> for DescriptorTablePointer {
    fn from(descriptor_table: &T) -> Self {
        Self {
            limit: descriptor_table.limit(),
            base: core::ptr::from_ref(descriptor_table).addr(),
        }
    }
}

impl core::fmt::Debug for DescriptorTablePointer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Safety: Pointer is a direct reference.
        let limit = unsafe { (&raw const self.limit).read_unaligned() };
        // Safety: Pointer is a direct reference.
        let base = unsafe { (&raw const self.base).read_unaligned() };

        f.debug_struct("DescriptorTablePointer")
            .field("Limit", &limit)
            .field("Base", &base)
            .finish()
    }
}
