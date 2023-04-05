mod scheduling;
use alloc::collections::VecDeque;
pub use scheduling::*;

mod artifact;
pub use artifact::*;

pub mod task;

pub static TASKS: spin::Mutex<VecDeque<task::Task>> = spin::Mutex::new(VecDeque::new());
