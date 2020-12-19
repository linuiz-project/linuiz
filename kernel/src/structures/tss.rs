use crate::Address;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub privilege_stack_table: [Address; 3],
    reserved_2: u64,
    pub interrupt_stack_table: [Address; 7],
    reserved_3: u64,
    reserved_4: u16,
    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            privilege_stack_table: [Address::Virtual(0x0); 3],
            interrupt_stack_table: [Address::Virtual(0x0); 7],
            iomap_base: 0,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
            reserved_4: 0,
        }
    }
}
