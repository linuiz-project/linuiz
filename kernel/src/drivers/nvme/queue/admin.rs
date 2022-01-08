use crate::drivers::nvme::command::{admin::Admin, Command, Completion};
use core::mem::MaybeUninit;
use libstd::{addr_ty::Physical, memory::volatile::VolatileCell, Address, IndexRing, ReadWrite};

const CAP_OFFSET: usize = 0x0;
const AQA_OFFSET: usize = 0x24;
const ASQ_OFFSET: usize = 0x28;
const ACQ_OFFSET: usize = 0x30;

pub struct CompletionQueue<'q> {
    phys_addr: Address<Physical>,
    entries: alloc::boxed::Box<[MaybeUninit<Completion>]>,
    entry_count: u16,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
    phase_tag: bool,
    cur_index: IndexRing,
}

impl<'q> CompletionQueue<'q> {
    pub fn new(reg0: &'q libstd::memory::MMIO, entry_count: u16) -> Self {
        let size_in_bytes = (entry_count as usize) * core::mem::size_of::<Completion>();
        let size_in_frames = libstd::align_up_div(size_in_bytes, 0x1000);

        unsafe {
            let (phys_addr, mut alloc) = libstd::memory::malloc::try_get()
                .unwrap()
                .alloc_contiguous(size_in_frames)
                .expect(
                    "Failed to allocate contiguous memory for an administrative completion queue.",
                );
            alloc.clear();

            let dstrd = reg0
                .borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
                .get_dstrd();
            let doorbell_offset =
                super::get_doorbell_offset(0, super::QueueType::Completion, dstrd as usize);

            use bit_field::BitField;
            reg0.write(
                AQA_OFFSET,
                *reg0
                    .read::<u32>(AQA_OFFSET)
                    .assume_init()
                    .set_bits(16.., entry_count as u32),
            );
            reg0.write(ACQ_OFFSET, phys_addr.as_usize());

            Self {
                phys_addr,
                entries: alloc.cast().unwrap().into_slice(),
                entry_count,
                doorbell: reg0.borrow(doorbell_offset),
                cur_index: IndexRing::new(entry_count as usize),
                phase_tag: true,
            }
        }
    }

    fn entries(&self) -> &[MaybeUninit<Completion>] {
        &self.entries[..(self.entry_count as usize)]
    }
}

impl core::fmt::Debug for CompletionQueue<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Admin Completion Queue")
            .field("Physical Address", &self.phys_addr)
            .field("Entries", &self.entries())
            .field("Index", &self.cur_index)
            .field("Phase Tag", &self.phase_tag)
            .finish()
    }
}

pub struct SubmissionQueue<'q> {
    phys_addr: Address<Physical>,
    entries: alloc::boxed::Box<[MaybeUninit<Command<Admin>>]>,
    entry_count: u16,
    cur_index: IndexRing,
    pending_count: u16,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
}

impl<'q> SubmissionQueue<'q> {
    pub fn new(reg0: &'q libstd::memory::MMIO, entry_count: u16) -> Self {
        // TODO somehow validate entry size?

        let size_in_bytes = (entry_count as usize) * core::mem::size_of::<Command<Admin>>();
        let size_in_frames = libstd::align_up_div(size_in_bytes, 0x1000);

        unsafe {
            let (phys_addr, mut alloc) = libstd::memory::malloc::try_get()
                .unwrap()
                .alloc_contiguous(size_in_frames)
                .expect(
                    "Failed to allocate contiguous memory for an administrative completion queue.",
                );
            alloc.clear();

            let dstrd = reg0
                .borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
                .get_dstrd();
            let doorbell_offset =
                super::get_doorbell_offset(0, super::QueueType::Submission, dstrd as usize);

            use bit_field::BitField;
            reg0.write(
                AQA_OFFSET,
                *reg0
                    .read::<u32>(AQA_OFFSET)
                    .assume_init()
                    .set_bits(..16, entry_count as u32),
            );
            reg0.write(ASQ_OFFSET, phys_addr.as_usize());

            Self {
                phys_addr,
                doorbell: reg0.borrow(doorbell_offset),
                entries: alloc.cast().unwrap().into_slice(),
                entry_count,
                cur_index: IndexRing::new(entry_count as usize),
                pending_count: 0,
            }
        }
    }

    fn entries(&self) -> &[MaybeUninit<Command<Admin>>] {
        &self.entries[..(self.entry_count as usize)]
    }

    // pub fn next_entry<T: super::command::CommandType<Q>>(
    //     &self,
    // ) -> Option<&mut super::command::Command<Q, T>> {
    //     if self.pending_entries.load(Ordering::Acquire) < self.entry_count {
    //         // transmute the &mut [u32] to an &mut Command
    //         let entry: &mut super::command::Command<Q, T> = unsafe {
    //             &mut *(self
    //                 .entries
    //                 .add(self.cur_index.load(Ordering::Acquire) as usize)
    //                 as *mut _)
    //         };
    //         self.pending_entries.fetch_add(1, Ordering::AcqRel);
    //         if self.cur_index.load(Ordering::Acquire) == self.entry_count {
    //             self.cur_index.store(0, Ordering::Release);
    //         } else {
    //             self.cur_index.fetch_add(1, Ordering::AcqRel);
    //         }
    //         entry.clear();
    //         // Automatically set opcode for command type T.
    //         entry.set_opcode(T::OPCODE);
    //         Some(entry)
    //     } else {
    //         None
    //     }
    // }

    pub fn flush_commands(&mut self) {
        self.doorbell.write(self.cur_index.index() as u32);
        self.pending_count = 0;
    }
}

impl core::fmt::Debug for SubmissionQueue<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Admin Submission Queue")
            .field("Physical Address", &self.phys_addr)
            .field("Entries", &self.entries())
            .field("Index", &self.cur_index)
            .field("Pending", &self.pending_count)
            .finish()
    }
}
