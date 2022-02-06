pub static mut TSS_STACK_PTRS: [Option<*const ()>; 7] = [None; 7];

lazy_static::lazy_static! {
    pub static ref TSS: x86_64::structures::tss::TaskStateSegment = {
        let mut tss = x86_64::structures::tss::TaskStateSegment::new();

        for (index, virt_addr) in unsafe { TSS_STACK_PTRS }
                .iter()
                .enumerate()
                .filter_map(|(index, ptr)| ptr.map(|ptr| (index, x86_64::VirtAddr::from_ptr(ptr)))) {
            tss.interrupt_stack_table[index] = virt_addr;
        }

        tss
    };
}
