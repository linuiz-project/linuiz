use core::cell::SyncUnsafeCell;
use spin::Mutex;
use x86_64::structures::idt::InterruptDescriptorTable;

static IDT: SyncUnsafeCell<Mutex<InterruptDescriptorTable>> =
    SyncUnsafeCell::new(Mutex::new(InterruptDescriptorTable::new()));

/// Helper function for immutably accessing the IDT mutex.
#[inline]
fn get_idt() -> &'static Mutex<InterruptDescriptorTable> {
    unsafe { &*IDT.get() }
}

/// Initialize the global IDT's exception and stub handlers.
pub fn init_idt() {
    assert!(
        crate::tables::gdt::KCODE_SELECTOR.get().is_some(),
        "Cannot initialize IDT before GDT (IDT entries use GDT kernel code segment selector)."
    );

    let mut idt = get_idt().lock();

    crate::interrupts::set_exception_handlers(&mut *idt);
    crate::interrupts::set_stub_handlers(&mut *idt);
}

/// Loads the global IDT using `lidt`.
pub fn load_idt() {
    let idt = get_idt().lock();
    unsafe { idt.load_unsafe() };
}
