use super::command::{Command, CommandResult, QueueEntry};
use alloc::boxed::Box;
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicU16, Ordering},
};
use lzstd::{
    memory::{volatile::VolatileCell, PageAlignedBox},
    Address, IndexRing, ReadWrite, {Physical, Virtual},
};

/// Uses the NVMe specified equation to calculate the offset of a particular queue's doorbell.
///
/// REMARK: `ty_mult` is the type multiplier for the queue type. `Submission` is 0, `Completion` is 1.
const fn calc_doorbell_offset(queue_id: u16, ty_mult: usize, dstrd: usize) -> usize {
    0x1000 + ((((queue_id as usize) * 2) + (ty_mult as usize)) * (4 << dstrd))
}

const CAP_OFFSET: usize = 0x0;
const AQA_OFFSET: usize = 0x24;
const ASQ_OFFSET: usize = 0x28;
const ACQ_OFFSET: usize = 0x30;

#[derive(Debug)]
pub struct QueueOverflow;

pub trait QueueType {
    type EntryType: QueueEntry;
    const DOORBELL_OFFSET: usize;
}

pub struct Submission;
impl QueueType for Submission {
    type EntryType = Command;
    const DOORBELL_OFFSET: usize = 0;
}

pub struct Completion;
impl QueueType for Completion {
    type EntryType = CommandResult;
    const DOORBELL_OFFSET: usize = 1;
}

pub struct Queue<'q, T: QueueType> {
    entries: lzstd::memory::PageAlignedBox<[MaybeUninit<T::EntryType>]>,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
    cur_index: IndexRing,
    phase_tag: bool, /* this is unused for submission queues */
}

impl<'q, T: QueueType> Queue<'q, T> {
    /// Creates a new NVMe queue of type `T`.
    ///
    /// REMARK: The `queue_id` provided is explicitly trusted. For admin queues, it should always
    ///         be 0. For all other queues, if the queue ID is already used for its respective type,
    ///         then an erroring CommandResult is returned by the controller.
    pub fn new(
        reg0: &'q lzstd::memory::MMIO,
        queue_id: u16, /* may need a way to dynamically select this? */
        entry_count: u16,
    ) -> Self {
        unsafe {
            let doorbell_offset = calc_doorbell_offset(
                queue_id,
                T::DOORBELL_OFFSET,
                reg0.borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET).get_dstrd() as usize,
            );

            Self {
                entries: Box::new_uninit_slice_in(entry_count as usize, lzstd::memory::page_aligned_allocator()),
                doorbell: reg0.borrow(doorbell_offset),
                cur_index: IndexRing::new(entry_count as usize),
                phase_tag: true,
            }
        }
    }

    pub fn get_phys_addr(&self) -> Address<Physical> {
        Address::<Physical>::new(
            lzstd::memory::global_pmgr()
                .get_mapped_to(&lzstd::memory::Page::from_ptr(self.entries.as_ptr()))
                .unwrap(),
        )
    }
}

/* COMPLETION QUEUE */

impl Queue<'_, Completion> {
    /// Provides the next `CommandResult`, or `None`.
    pub fn next_cmd_result(&mut self) -> Option<CommandResult> {
        // Assume the current completion is initialized, to test the Phase Tag bit.
        let cur_completion = unsafe { self.entries[self.cur_index.index()].assume_init() };
        info!("CHECKING COMPLETION: {:?}", cur_completion);
        // Test the current completion's phase tag bit against our own.
        //
        // Our phase tag bit should always be inverted to newly incoming completions.
        //
        // REMARK: It may be possible to fail processing sufficient completions
        //         before the next interrupt is sent, thus resulting in this approach
        //         not processing all of the queued completions.
        if cur_completion.get_phase_tag() != self.phase_tag {
            self.doorbell.write(self.cur_index.index() as u32);
            self.cur_index.increment();
            Some(cur_completion)
        } else {
            None
        }
    }

    /// Inverts the phase tag match for the queue. This should only be done
    /// when
    pub fn invert_phase(&mut self) {
        self.phase_tag = !self.phase_tag;
    }

    /// Increments the completion queue index. Then, if incrementing rolls over
    /// the index ring, the phase tag for this queue is inverted.
    fn increment_with_phase_inversion(&mut self) {
        self.cur_index.increment();

        if self.cur_index.index() == 0 {
            self.phase_tag = !self.phase_tag;
        }
    }
}

impl core::fmt::Debug for Queue<'_, Completion> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Admin Completion Queue")
            .field("Physical Address", &self.get_phys_addr())
            .field("Index", &self.cur_index)
            .field("Phase Tag", &self.phase_tag)
            .finish()
    }
}

/* SUBMISSION QUEUE */

impl Queue<'_, Submission> {
    pub fn submit_command(&mut self, command: Command) {
        self.entries[self.cur_index.index()] = MaybeUninit::new(command);
        self.cur_index.increment();
    }

    // TODO submit_commands function
}

impl core::fmt::Debug for Queue<'_, Submission> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Admin Submission Queue")
            .field("Physical Address", &self.get_phys_addr())
            .field("Index", &self.cur_index)
            .finish()
    }
}

// /// NVMe completion queue, used for receiving and tracking completed commands.
// pub(super) struct CompletionQueue<'q> {
//     entries: Box<QueueEntries<Completion>>,
//     doorbell: &'q VolatileCell<u32, ReadWrite>,
//     phase_tag: bool,
//     cur_index: IndexRing,
// }

// impl<'q> CompletionQueue<'q> {
//     pub fn new(
//         reg0: &'q lzstd::memory::MMIO,
//         queue_id: u16, /* may need a way to dynamically select this? */
//     ) -> Self {
//         unsafe {
//             let doorbell_offset = calc_doorbell_offset(
//                 queue_id,
//                 QueueType::Completion,
//                 reg0.borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
//                     .get_dstrd() as usize,
//             );

//             Self {
//                 entries: Box::new(QueueEntries::uninit()),
//                 doorbell: reg0.borrow(doorbell_offset),
//                 cur_index: IndexRing::new(GLOBAL_QUEUE_ENTRIES_COUNT),
//                 phase_tag: true,
//             }
//         }
//     }

//     pub fn get_phys_addr(&self) -> Address<Physical> {
//         Address::<Physical>::new(
//             lzstd::memory::global_pmgr()
//                 .get_mapped_to(&lzstd::memory::Page::from_ptr(self.entries.as_ptr()))
//                 .unwrap(),
//         )
//     }

//     pub fn next_completion(&mut self) -> Option<Completion> {
//         // Assume the current completion is initialized, to test the Phase Tag bit.
//         let cur_completion = unsafe { self.entries[self.cur_index.index()].assume_init() };

//         info!("CHECKING COMPLETION: {:?}", cur_completion);

//         // Test the current completion's phase tag bit against our own.
//         //
//         // Our phase tag bit should always be inverted to newly incoming completions.
//         //
//         // REMARK: It may be possible to fail processing sufficient completions
//         //         before the next interrupt is sent, thus resulting in this approach
//         //         not processing all of the queued completions.
//         if cur_completion.get_phase_tag() != self.phase_tag {
//             self.doorbell.write(self.cur_index.index() as u32);
//             self.cur_index.increment();

//             Some(cur_completion)
//         } else {
//             None
//         }
//     }

//     pub fn invert_phase(&mut self) {
//         self.phase_tag = !self.phase_tag;
//     }
// }

// impl core::fmt::Debug for CompletionQueue<'_> {
//     fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         formatter
//             .debug_struct("Admin Completion Queue")
//             .field("Physical Address", &self.get_phys_addr())
//             .field("Index", &self.cur_index)
//             .field("Phase Tag", &self.phase_tag)
//             .finish()
//     }
// }

// pub(super) struct SubmissionQueue<'q> {
//     entries: Box<QueueEntries<Command>>,
//     cur_index: IndexRing,
//     pending_count: u16,
//     doorbell: &'q VolatileCell<u32, ReadWrite>,
// }

// impl<'q> SubmissionQueue<'q> {
//     pub fn new(reg0: &'q lzstd::memory::MMIO, queue_id: u16) -> Self {
//         // TODO somehow validate entry size?

//         unsafe {
//             let doorbell_offset = calc_doorbell_offset(
//                 queue_id,
//                 QueueType::Submission,
//                 reg0.borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
//                     .get_dstrd() as usize,
//             );

//             Self {
//                 doorbell: reg0.borrow(doorbell_offset),
//                 entries: Box::new(QueueEntries::uninit()),
//                 cur_index: IndexRing::new(GLOBAL_QUEUE_ENTRIES_COUNT),
//                 pending_count: 0,
//             }
//         }
//     }

//     pub fn get_phys_addr(&self) -> Address<Physical> {
//         Address::<Physical>::new(
//             lzstd::memory::global_pmgr()
//                 .get_mapped_to(&lzstd::memory::Page::from_ptr(self.entries.as_ptr()))
//                 .unwrap(),
//         )
//     }

//     pub fn submit_command(&mut self, command: Command) -> Result<(), QueueOverflow> {
//         if self.pending_count < self.entry_count {
//             self.entries[self.cur_index.index()] = MaybeUninit::new(command);
//             self.cur_index.increment();
//             self.pending_count += 1;

//             Ok(())
//         } else {
//             Err(QueueOverflow)
//         }
//     }

//     pub fn flush_commands(&mut self) {
//         self.doorbell.write(self.cur_index.index() as u32);
//         self.pending_count = 0;
//     }
// }

// impl core::fmt::Debug for SubmissionQueue<'_> {
//     fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         formatter
//             .debug_struct("Admin Submission Queue")
//             .field("Physical Address", &self.get_phys_addr())
//             .field("Index", &self.cur_index)
//             .field("Pending", &self.pending_count)
//             .finish()
//     }
// }
