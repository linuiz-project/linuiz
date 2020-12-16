use core::marker::PhantomData;

// todo this before IDT

pub struct InterruptVector<F> {
    pointer_low: u16,
    gdt_selector: u16,
    options: ---,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
    phantom: PhantomData<F>
}

