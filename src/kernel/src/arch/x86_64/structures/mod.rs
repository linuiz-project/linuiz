pub mod gdt;
pub mod idt;
pub mod tss;

/// A struct describing a pointer to a descriptor table (GDT / IDT).
/// This is in a format suitable for giving to 'lgdt' or 'lidt'.
#[repr(C, packed(2))]
#[derive(Clone, Copy)]
pub struct DescriptorTablePointer<T> {
    /// Size of the DT in bytes, less 1.
    limit: u16,

    /// Memory offset (pointer) to the table.
    base: *const T,
}

impl<T> From<&T> for DescriptorTablePointer<T> {
    fn from(value: &T) -> Self {
        Self {
            limit: u16::try_from(size_of::<T>() - 1).unwrap(),
            base: core::ptr::from_ref(value),
        }
    }
}

impl<T> core::fmt::Debug for DescriptorTablePointer<T> {
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
