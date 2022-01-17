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
    Submission = 0,
    Completion = 1,
}

pub(self) fn get_doorbell_offset(queue_id: u16, ty: QueueType, dstrd: usize) -> usize {
    0x1000 + ((((queue_id as usize) * 2) + (ty as usize)) * (4 << dstrd))
}
