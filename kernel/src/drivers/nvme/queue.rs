// use super::command::{Completion, NVME_COMMAND};
// use core::{
//     mem::MaybeUninit,
//     sync::atomic::{AtomicU16, Ordering},
// };
// use libstd::{
//     addr_ty::{Physical, Virtual},
//     memory::{falloc, malloc, volatile::VolatileCell},
//     Address, ReadWrite,
// };

// pub trait QueueDomain {}
// pub enum Admin {}
// impl QueueDomain for Admin {}
// pub enum IO {}
// impl QueueDomain for IO {}

// pub struct CompletionQueue<'q> {
//     frames: libstd::memory::FrameIterator,
//     entries: &'q [MaybeUninit<Completion>],
//     doorbell: &'q VolatileCell<u32, ReadWrite>,
//     cur_index: u16,
//     phase_tag: bool,
// }

// impl<'q> CompletionQueue<'q> {
//     pub fn new(doorbell_addr: Address<Virtual>, entry_count: u16, entry_size: usize) -> Self {
//         // TODO somehow validate entry size

//         let size_in_bytes = (entry_count as usize) * entry_size;
//         let min_frame_count = libstd::align_up_div(size_in_bytes, 0x1000);
//         let alloc = malloc::try_get().unwrap()
//             .alloc_contiguous(min_frame_count)
//             .expect("Allocation error while attempting to create queue.")
//             .1;

//         unsafe {
//             alloc.as_uninit_slice_mut().fill(MaybeUninit::new(0));
//             //let entries = alloc.cast() &mut *core::slice::from_raw_parts_mut(alloc, entry_count as usize);

//             Self {
//                 frames,
//                 doorbell: &*doorbell_addr.as_ptr(),
//                 entries: alloc.cast().as_uninit_slice(),
//                 cur_index: 0,
//                 phase_tag: true,
//             }
//         }
//     }

//     pub fn base_phys_addr(&self) -> Address<Physical> {
//         self.frames.start().base_addr()
//     }

//     fn increment_index(&mut self) {
//         if (self.cur_index as usize) == (self.entries.len() - 1) {
//             self.phase_tag = !self.phase_tag;
//             self.cur_index = 0;
//         } else {
//             self.cur_index += 1;
//         }

//         self.doorbell.write(self.cur_index as u32);
//     }

//     pub fn next_entry(&mut self) -> Option<&super::command::Completion> {
//         let completion = &self.entries[self.cur_index as usize];

//         if completion.phase_tag() == self.phase_tag {
//             self.increment_index();
//             Some(completion)
//         } else {
//             None
//         }
//     }
// }

// // TODO make this thread-safe
// pub struct SubmissionQueue<'q, Q: QueueDomain> {
//     frames: libstd::memory::FrameIterator,
//     entries: *mut NVME_COMMAND,
//     entry_count: u16,
//     doorbell: &'q VolatileCell<u32, ReadWrite>,
//     cur_index: AtomicU16,
//     unsubmitted_entries: AtomicU16,
//     phantom: core::marker::PhantomData<Q>,
// }

// impl<'q, Q: QueueDomain> SubmissionQueue<'q, Q> {
//     pub fn new(doorbell_addr: Address<Virtual>, entry_count: u16, entry_size: usize) -> Self {
//         // TODO somehow validate entry size

//         let size_in_bytes = (entry_count as usize) * entry_size;
//         let min_frame_count = libstd::align_up_div(size_in_bytes, 0x1000);
//         let alloc_addr = malloc::try_get().unwrap().alloc_contiguous(&frames);

//         unsafe {
//             core::ptr::write_bytes(alloc_addr as *mut u8, 0, size_in_bytes);

//             Self {
//                 frames,
//                 doorbell: &*doorbell_addr.as_ptr(),
//                 entries: alloc_addr,
//                 entry_count,
//                 cur_index: AtomicU16::new(0),
//                 unsubmitted_entries: AtomicU16::new(0),
//                 phantom: core::marker::PhantomData,
//             }
//         }
//     }

//     pub fn base_phys_addr(&self) -> Address<Physical> {
//         self.frames.start().base_addr()
//     }

//     fn increment_index(&self) {}

//     pub fn next_entry<T: super::command::CommandType<Q>>(
//         &self,
//     ) -> Option<&mut super::command::Command<Q, T>> {
//         if self.unsubmitted_entries.load(Ordering::Acquire) < self.entry_count {
//             // transmute the &mut [u32] to an &mut Command
//             let entry: &mut super::command::Command<Q, T> = unsafe {
//                 &mut *(self
//                     .entries
//                     .add(self.cur_index.load(Ordering::Acquire) as usize)
//                     as *mut _)
//             };

//             self.unsubmitted_entries.fetch_add(1, Ordering::AcqRel);
//             if self.cur_index.load(Ordering::Acquire) == self.entry_count {
//                 self.cur_index.store(0, Ordering::Release);
//             } else {
//                 self.cur_index.fetch_add(1, Ordering::AcqRel);
//             }

//             entry.clear();
//             // Automatically set opcode for command type T.
//             entry.set_opcode(T::OPCODE);
//             Some(entry)
//         } else {
//             None
//         }
//     }

//     pub fn flush_commands(&self) {
//         self.doorbell
//             .write(self.cur_index.load(Ordering::Acquire) as u32);
//         self.unsubmitted_entries.store(0, Ordering::Release);
//     }
// }

// // TODO implement this
// // impl Drop for SubmissionQueue<'_> {
// //     fn drop(&mut self) {
// //         unsafe {
// //             libstd::memory::malloc::try_get().unwrap().dealloc(
// //                 self.entries.as_mut_ptr() as *mut u8,
// //                 core::alloc::Layout::from_size_align_unchecked(
// //                     self.entries.len() * core::mem::size_of::<[u32; COMMAND_DWORDS]>(),
// //                     1,
// //                 ),
// //             )
// //         }
// //     }
// // }
