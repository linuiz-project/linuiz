use crate::{cpu::context::Context, mem::AddressSpace};
use uuid::Uuid;

#[derive(Debug)]
pub enum Task {
    Procedure {
        id: Uuid,
        context: Context,
    },
    Process {
        id: Uuid,
        context: Context,
        address_space: AddressSpace,
        load_offset: usize,
        // TODO elf information
    },
}

impl Task {
    pub const fn id(&self) -> Uuid {
        match self {
            Task::Procedure { id, context: _ }
            | Task::Process {
                id,
                context: _,
                address_space: _,
                load_offset: _,
            } => *id,
        }
    }

    pub const fn context(&self) -> &Context {
        match self {
            Task::Procedure { id: _, context }
            | Task::Process {
                id: _,
                context,
                address_space: _,
                load_offset: _,
            } => context,
        }
    }

    pub const fn context_mut(&mut self) -> &mut Context {
        match self {
            Task::Procedure { id: _, context }
            | Task::Process {
                id: _,
                context,
                address_space: _,
                load_offset: _,
            } => context,
        }
    }
}
