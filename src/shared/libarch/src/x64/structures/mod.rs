pub mod apic;
pub mod gdt;
pub mod idt;
pub mod ioapic;

pub fn load_static_tables() {
    use crate::x64::{
        instructions::tables,
        structures::{
            gdt::Descriptor,
            idt::{InterruptDescriptorTable, StackTableIndex},
        },
    };
    use x86_64::{structures::tss::TaskStateSegment, VirtAddr};

    trace!("Loading and configuring static kernel tables.");

    // Always initialize GDT prior to configuring IDT.
    crate::x64::structures::gdt::init();

    /*
     * IDT
     * Due to the fashion in which the `x86_64` crate initializes the IDT entries,
     * it must be ensured that the handlers are set only *after* the GDT has been
     * properly initialized and loaded—otherwise, the `CS` value for the IDT entries
     * is incorrect, and this causes very confusing GPFs.
     */
    {
        static LOW_MEMORY_IDT: spin::Lazy<InterruptDescriptorTable> = spin::Lazy::new(|| {
            let mut idt = InterruptDescriptorTable::new();
            crate::x64::structures::idt::set_exception_handlers(idt_ptr.as_mut());
            crate::x64::structures::idt::set_stub_handlers(idt_ptr.as_mut());
            idt
        });

        LOW_MEMORY_IDT.load();
    }
}

/// SAFETY: Caller must ensure this method is called only once per core.
pub unsafe fn load_local_tables() {
    // TODO move this to local state init?
    let mut idt_ptr = alloc::alloc::alloc_zeroed(unsafe {
        core::alloc::Layout::from_size_align_unchecked(
            core::mem::size_of::<InterruptDescriptorTable>(),
            core::mem::align_of::<InterruptDescriptorTable>(),
        )
    });

    let idt = idt_ptr.as_mut().unwrap();
    crate::x64::structures::idt::set_exception_handlers(idt_ptr.as_mut());
    crate::x64::structures::idt::set_stub_handlers(idt_ptr.as_mut());
    idt_ptr.load_unsafe();

    /* TSS init */
    {
        trace!("Configuring new TSS and loading via temp GDT.");

        let tss_ptr = {
            use core::mem::MaybeUninit;
            use libcommon::memory::stack_aligned_allocator;

            let mut tss = Box::new(TaskStateSegment::new());

            let allocate_tss_stack = |pages: usize| {
                VirtAddr::from_ptr::<MaybeUninit<()>>(
                    Box::leak(Box::new_uninit_slice_in(pages * 0x1000, stack_aligned_allocator())).as_ptr(),
                )
            };

            // TODO guard pages for these stacks ?
            tss.privilege_stack_table[0] = allocate_tss_stack(5);
            tss.interrupt_stack_table[StackTableIndex::Debug as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] = allocate_tss_stack(2);
            tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] = allocate_tss_stack(2);

            Box::leak(tss) as *mut _
        };

        trace!("Configuring TSS descriptor for temp GDT.");
        let tss_descriptor = {
            use bit_field::BitField;

            let tss_ptr_u64 = tss_ptr as u64;

            let mut low = x86_64::structures::gdt::DescriptorFlags::PRESENT.bits();
            // base
            low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
            low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
            // limit (the `-1` is needed since the bound is inclusive, not exclusive)
            low.set_bits(0..16, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);
            // type (0b1001 = available 64-bit tss)
            low.set_bits(40..44, 0b1001);

            // high 32 bits of base
            let mut high = 0;
            high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

            Descriptor::SystemSegment(low, high)
        };

        trace!("Loading in temp GDT to `ltr` the TSS.");
        // Store current GDT pointer to restore later.
        let cur_gdt = tables::sgdt();
        // Create temporary kernel GDT to avoid a GPF on switching to it.
        let mut temp_gdt = x86_64::structures::gdt::GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);

        // Load temp GDT ...
        temp_gdt.load_unsafe();
        // ... load TSS from temporary GDT ...
        tables::load_tss(tss_selector);
        // ... and restore cached GDT.
        tables::lgdt(&cur_gdt);

        trace!("TSS loaded, and temporary GDT trashed.");
    }
}
