use spin::Lazy;

pub mod apic;
pub mod gdt;
pub mod idt;
pub mod ioapic;
pub mod tss;

pub fn load_static_tables() {
    use crate::x64::structures::idt::InterruptDescriptorTable;

    trace!("Loading and configuring static kernel tables.");

    // Always initialize GDT prior to configuring IDT.
    crate::x64::structures::gdt::load_kernel();

    /*
     * IDT
     * Due to the fashion in which the `x86_64` crate initializes the IDT entries,
     * it must be ensured that the handlers are set only *after* the GDT has been
     * properly initialized and loaded—otherwise, the `CS` value for the IDT entries
     * is incorrect, and this causes very confusing GPFs.
     */
    {
        static LOW_MEMORY_IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
            let mut idt = InterruptDescriptorTable::new();
            crate::x64::structures::idt::set_exception_handlers(&mut idt);
            crate::x64::structures::idt::set_stub_handlers(&mut idt);
            idt
        });

        LOW_MEMORY_IDT.load();
    }
}
