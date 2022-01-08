use core::marker::PhantomData;

use alloc::boxed::Box;
use libstd::{addr_ty::Physical, Address};

pub enum Admin {}
impl super::QueueDomain for Admin {}

#[repr(u8)]
pub enum Opcode {
    DeleteIOSubmissionQueue = 0x0,
    CreateIOSubmissionQueue = 0x1,
    GetLogPage = 0x2,
    DeleteIOCompletionQueue = 0x4,
    CreateIOCompletionQueue = 0x5,
    Identify = 0x6,
    Abort = 0x8,
    SetFeatures = 0x9,
    GetFeatures = 0xA,
    AsyncEventRequest = 0xC,
    // TODO
}

impl super::Command<Admin> {
    pub const fn create_io_completion_queue(
        id: u16,
        len: u16,
        queue_ptr: super::DataPointer,
        phys_contiguous: bool,
        int_vector: Option<u16>,
    ) -> Self {
        Self {
            opcode: Opcode::CreateIOCompletionQueue as u8,
            fuse_psdt: ((super::PSDT::PRP as u8) << 14) | (super::FuseOperation::Normal as u8),
            ns_id: 0,
            cdw2: 0,
            cdw3: 0,
            mdata_ptr: Address::zero(),
            data_ptr: queue_ptr.as_u128(),
            cdw10: ((len << 16) as u32) | (id as u32),
            cdw11: match int_vector {
                Some(vector) => ((vector as u32) << 16) | (1 << 1) | (phys_contiguous as u32),
                None => phys_contiguous as u32,
            },
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
            marker: PhantomData,
        }
    }

    pub fn identify(ctrl_id: u16) -> (Box<core::mem::MaybeUninit<Identify>>, Self) {
        let (phys_addr, alloc) = unsafe {
            libstd::memory::malloc::try_get()
                .unwrap()
                .alloc_contiguous(1)
                .unwrap()
        };

        (
            alloc.cast().unwrap().into_uninit_value().unwrap(),
            Self {
                opcode: Opcode::Identify as u8,
                fuse_psdt: ((super::PSDT::PRP as u8) << 14) | (super::FuseOperation::Normal as u8),
                ns_id: 0,
                cdw2: 0,
                cdw3: 0,
                mdata_ptr: Address::zero(),
                data_ptr: super::DataPointer::PRP(phys_addr, Address::zero()).as_u128(),
                cdw10: ((ctrl_id as u32) << 16) | 0, // TODO implement CNS
                cdw11: 0, // Ensure CSI or CNS Specific Identifier are not required,
                cdw12: 0,
                cdw13: 0,
                cdw14: 0, // Ensure no UUID is required, or possibly allow providing one (?)
                cdw15: 0,
                marker: PhantomData,
            },
        )
    }
}

#[repr(C)]
pub struct Identify {
    rsdv0: [u8; 80],
    version: [u8; 3],
    rsvd1: [u8; 4096 - 83],
}
