use super::command::{Command, Completion};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicU16, Ordering},
};
use lib::{
    addr_ty::{Physical, Virtual},
    memory::{malloc, volatile::VolatileCell},
    Address, IndexRing, ReadWrite,
};

#[repr(usize)]
enum QueueType {
    Submission = 0,
    Completion = 1,
}

const fn get_doorbell_offset(queue_id: u16, ty: QueueType, dstrd: usize) -> usize {
    0x1000 + ((((queue_id as usize) * 2) + (ty as usize)) * (4 << dstrd))
}

const CAP_OFFSET: usize = 0x0;
const AQA_OFFSET: usize = 0x24;
const ASQ_OFFSET: usize = 0x28;
const ACQ_OFFSET: usize = 0x30;

#[derive(Debug)]
pub struct QueueOverflow;

pub(super) struct CompletionQueue<'q> {
    phys_addr: Address<Physical>,
    entries: alloc::boxed::Box<[MaybeUninit<Completion>]>,
    entry_count: u16,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
    phase_tag: bool,
    cur_index: IndexRing,
}

impl<'q> CompletionQueue<'q> {
    pub fn new(
        reg0: &'q lib::memory::MMIO,
        queue_id: u16, /* may need a way to dynamically select this? */
        entry_count: u16,
    ) -> Self {
        let size_in_bytes = (entry_count as usize) * core::mem::size_of::<Completion>();
        let size_in_frames = lib::align_up_div(size_in_bytes, 0x1000);

        unsafe {
            let (phys_addr, mut alloc) = lib::memory::malloc::get()
                .alloc_contiguous(size_in_frames)
                .expect(
                    "Failed to allocate contiguous memory for an administrative completion queue.",
                );
            alloc.clear();

            let doorbell_offset = get_doorbell_offset(
                queue_id,
                QueueType::Completion,
                reg0.borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
                    .get_dstrd() as usize,
            );

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

    pub const fn phys_addr(&self) -> Address<Physical> {
        self.phys_addr
    }

    pub fn next_completion(&mut self) -> Option<Completion> {
        // Assume the current completion is initialized, to test the Phase Tag bit.
        let cur_completion = unsafe { self.entries()[self.cur_index.index()].assume_init() };

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

    pub fn invert_phase(&mut self) {
        self.phase_tag = !self.phase_tag;
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

pub(super) struct SubmissionQueue<'q> {
    phys_addr: Address<Physical>,
    entries: alloc::boxed::Box<[MaybeUninit<Command>]>,
    entry_count: u16,
    cur_index: IndexRing,
    pending_count: u16,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
}

impl<'q> SubmissionQueue<'q> {
    pub fn new(reg0: &'q lib::memory::MMIO, queue_id: u16, entry_count: u16) -> Self {
        // TODO somehow validate entry size?

        let size_in_bytes = (entry_count as usize) * core::mem::size_of::<Command>();
        let size_in_frames = lib::align_up_div(size_in_bytes, 0x1000);

        unsafe {
            let (phys_addr, mut alloc) = lib::memory::malloc::get()
                .alloc_contiguous(size_in_frames)
                .expect(
                    "Failed to allocate contiguous memory for an administrative completion queue.",
                );
            alloc.clear();

            let doorbell_offset = get_doorbell_offset(
                queue_id,
                QueueType::Submission,
                reg0.borrow::<crate::drivers::nvme::Capabilities>(CAP_OFFSET)
                    .get_dstrd() as usize,
            );

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

    pub const fn phys_addr(&self) -> Address<Physical> {
        self.phys_addr
    }

    pub fn submit_command(&mut self, command: Command) -> Result<(), QueueOverflow> {
        if self.pending_count < self.entry_count {
            self.entries[self.cur_index.index()] = MaybeUninit::new(command);
            self.cur_index.increment();
            self.pending_count += 1;

            Ok(())
        } else {
            Err(QueueOverflow)
        }
    }

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
            .field("Entries", &self.entries)
            .field("Index", &self.cur_index)
            .field("Pending", &self.pending_count)
            .finish()
    }
}
