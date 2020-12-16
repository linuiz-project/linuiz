pub mod segment_selector;
pub mod segment_descriptor;

use segment_descriptor::{SegmentDescriptor, SegmentDescriptorFlags};
use segment_selector::SegmentSelector;
use crate::PrivilegeLevel;

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
                    PrivilegeLevel:: Ring1
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