pub static mut TSS_STACK_PTRS: [Option<*const ()>; 7] = [None; 7];

lazy_static::lazy_static! {
    pub static ref TSS: x86_64::structures::tss::TaskStateSegment = {
        let mut tss = x86_64::structures::tss::TaskStateSegment::new();

        unsafe {
            for (index, stack_ptr) in TSS_STACK_PTRS.iter().enumerate() {
                if let Some(stack_ptr) = stack_ptr {
                    tss.interrupt_stack_table[index] = x86_64::VirtAddr::from_ptr(stack_ptr);
                }
            }
        }

        tss
    };
}
