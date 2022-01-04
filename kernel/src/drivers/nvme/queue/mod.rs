pub mod admin;
pub mod io;

use super::command::Completion;
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicU16, Ordering},
};
use libstd::{
    addr_ty::{Physical, Virtual},
    memory::{falloc, malloc, volatile::VolatileCell},
    Address, ReadWrite,
};

#[repr(usize)]
pub enum QueueType {
    Completion = 0,
    Submission = 1,
}

pub(self) fn get_doorbell_offset(queue_id: u16, ty: QueueType, dstrd: usize) -> usize {
    0x1000 + ((((queue_id as usize) * 2) + (ty as usize)) * (4 << dstrd))
}

// TODO implement this
// impl Drop for SubmissionQueue<'_> {
//     fn drop(&mut self) {
//         unsafe {
//             libstd::memory::malloc::try_get().unwrap().dealloc(
//                 self.entries.as_mut_ptr() as *mut u8,
//                 core::alloc::Layout::from_size_align_unchecked(
//                     self.entries.len() * core::mem::size_of::<[u32; COMMAND_DWORDS]>(),
//                     1,
//                 ),
//             )
//         }
//     }
// }
