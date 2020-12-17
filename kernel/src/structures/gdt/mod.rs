pub mod segment_selector;
pub mod segment_descriptor;

use crate::{Address, PrivilegeLevel, structures::tss::TaskStateSegment};
use segment_descriptor::{SegmentDescriptor, SegmentDescriptorFlags};
use segment_selector::SegmentSelector;
use lazy_static::lazy_static;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector
}

#[derive(Debug, Clone)]
pub struct GlobalDescriptorTable {
    table: [u64; 8],
    next_free: usize
}

impl GlobalDescriptorTable {
    pub const fn new() -> Self {
        Self {
            table: [0; 8],
            next_free: 1
        }
    }

    pub const fn add_entry(&mut self, entry: SegmentDescriptor) -> SegmentSelector {
        let index = match entry {
            SegmentDescriptor::UserSegment(segment) => self.push(segment),
            SegmentDescriptor::SystemSegment(segment_low, segment_high) => {
                let index = self.push(segment_low);
                self.push(segment_high);
                index
            }
        };

        let rpl = match entry {
            SegmentDescriptor::UserSegment(segment) => {
                if SegmentDescriptorFlags::from_bits_truncate(segment).contains(SegmentDescriptorFlags::DPL_RING_3) {
                    PrivilegeLevel::Ring3
                } else if SegmentDescriptorFlags::from_bits_truncate(segment).contains(SegmentDescriptorFlags::DPL_RING_2) {
                    PrivilegeLevel::Ring2
                } else if SegmentDescriptorFlags::from_bits_truncate(segment).contains(SegmentDescriptorFlags::DPL_RING_1) {
                    PrivilegeLevel::Ring1
                } else {
                    PrivilegeLevel::Ring0
                }
            },
            SegmentDescriptor::SystemSegment(segment_low, segment_high) => PrivilegeLevel::Ring0
        };

        SegmentSelector::new(index as u16, rpl)
    }

    const fn push(&mut self, value: u64) -> usize {
        if self.next_free < self.table.len() {
            let index = self.next_free;
            self.table[index] = value;
            self.next_free += 1;
            index
        } else {
            panic!("GDT is full")
        }
    }
}

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = Address::from(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end

        };

        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(SegmentDescriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(SegmentDescriptor::tss_segment(&TSS));

    (gdt, Selectors {
        code_selector,
        tss_selector
    })
    };
}

pub fn init() {

}