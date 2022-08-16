use x86_64::structures::idt::InterruptDescriptorTable;

/// Stores the given IDT in the interrupt descriptor table register.
///
/// SAFETY: This function leaks the memory that the IDT is stored within, so it
///         is assumed the caller will only invoke this once per core.
///
/// TODO:   Move to per-core IDT.
pub unsafe fn store(idt: alloc::boxed::Box<InterruptDescriptorTable>) {
    let idt = alloc::boxed::Box::leak(idt);
    idt.load_unsafe();
}

/// Loads the IDT from the interrupt descriptor table register.
pub unsafe fn load() -> Option<&'static mut InterruptDescriptorTable> {
    let idt_pointer = x86_64::instructions::tables::sidt();
    idt_pointer.base.as_mut_ptr::<InterruptDescriptorTable>().as_mut()
}
