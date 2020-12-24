use crate::Address;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TaskStateSegment {
    reserved_1: u32,
    privilege_stack_table: [usize; 3],
    reserved_2: u64,
    interrupt_stack_table: [usize; 7],
    reserved_3: u64,
    reserved_4: u16,
    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            privilege_stack_table: [0x0; 3],
            interrupt_stack_table: [0x0; 7],
            iomap_base: 0,
            reserved_1: 0,
            reserved_2: 0,
            reserved_3: 0,
            reserved_4: 0,
        }
    }

    pub fn set_pst_entry(&mut self, index: usize, address: Address) {
        self.interrupt_stack_table[index] = address.as_usize();
    }

    pub fn set_ist_entry(&mut self, index: usize, address: Address) {
        self.interrupt_stack_table[index] = address.as_usize();
    }
}
