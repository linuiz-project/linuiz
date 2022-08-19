use x86_64::structures::idt::InterruptDescriptorTable;

/// Stores the given IDT in the interrupt descriptor table register.
///
/// SAFETY: This function assumes the given IDT will not be deallocated or
///         otherwise removed without proper preceding control flow.
pub unsafe fn store(idt: &mut InterruptDescriptorTable) {
    idt.load_unsafe();
}

/// Loads the IDT from the interrupt descriptor table register.
pub unsafe fn load() -> Option<&'static mut InterruptDescriptorTable> {
    let idt_pointer = x86_64::instructions::tables::sidt();
    idt_pointer.base.as_mut_ptr::<InterruptDescriptorTable>().as_mut()
}
