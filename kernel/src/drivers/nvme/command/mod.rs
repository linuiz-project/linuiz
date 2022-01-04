pub mod admin;
pub mod io;

use bit_field::BitField;
use core::{convert::TryFrom, fmt, marker::PhantomData, ops::Range};
use libstd::{
    addr_ty::Physical, memory::volatile::VolatileCell, volatile_bitfield_getter,
    volatile_bitfield_getter_as, Address, ReadWrite,
};
use num_enum::TryFromPrimitive;

#[repr(u8)]
#[derive(TryFromPrimitive)]
pub enum FuseOperation {
    Normal = 0b00,
    FirstCommand = 0b01,
    SecondCommand = 0b10,
}

#[repr(u8)]
pub enum PSDT {
    PRP = 0b00,
    SGLPhysBuffer = 0b01,
    SUGLDescriptor = 0b10,
    Reserved = 0b11,
}

#[repr(u32)]
pub enum DataTransfer {
    NoData = 0b00,
    HostToController = 0b01,
    ControllerToHost = 0b10,
    Bidirectional = 0b11,
}

pub enum DataPointer {
    PRP(Address<Physical>, Address<Physical>),
    SGL(u128),
}

impl DataPointer {
    pub const fn as_u128(self) -> u128 {
        match self {
            Self::PRP(addr1, addr2) => {
                ((addr2.as_usize() as u128) << 64) | (addr1.as_usize() as u128)
            }
            Self::SGL(sgl) => sgl,
        }
    }
}

pub trait QueueDomain {}
pub enum IO {}
impl QueueDomain for IO {}

#[repr(C)]
pub struct Command<Q: QueueDomain> {
    opcode: u8,
    fuse_psdt: u8,
    ns_id: u32,
    cdw2: u32,
    cdw3: u32,
    mdata_ptr: Address<Physical>,
    data_ptr: u128,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
    marker: core::marker::PhantomData<Q>,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum GenericStatus {
    SuccessfulCompletion = 0x0,
    InvalidCommandOpcode = 0x1,
    InvalidFieldInCommand = 0x2,
    CommandIDConflict = 0x3,
    DataTransferError = 0x4,
    PowerLossAbortNotification = 0x5,
    InternalError = 0x6,
    AbortRequested = 0x7,
    SubmissionQueueDeletionAbort = 0x8,
    FailedFuseAbort = 0x9,
    MissingFuseAbort = 0xA,
    InvalidNamespaceOrFormat = 0xB,
    CommandSequenceError = 0xC,
    InvalidSGLSegmentDescriptor = 0xD,
    InvalidSGLDescriptorCount = 0xE,
    InvalidSGLDataLength = 0xF,
    InvalidSGLMetadataLength = 0x10,
    InvalidSGLDescriptorType = 0x11,
    InvalidControllerMemoryBufferUsage = 0x12,
    PRPOffsetInvalid = 0x13,
    AtomicWriteUnitExceeded = 0x14,
}

#[repr(u32)]
#[derive(Debug)]
pub enum StatusCode {
    Generic(GenericStatus),
    CommandSpecific(u8),
    MediaAndDataIntegrityErrors, // 0x2
    PathRelatedStatus,           // 0x3
    VendorSpecific,              // 0x7
}

#[repr(transparent)]
pub struct CompletionStatus(u32);

impl CompletionStatus {
    pub fn dnr(&self) -> bool {
        self.0.get_bit(31)
    }

    pub fn more(&self) -> bool {
        self.0.get_bit(30)
    }

    pub fn status_code(&self) -> StatusCode {
        match self.0.get_bits(25..28) {
            0x0 => StatusCode::Generic(GenericStatus::try_from(self.0.get_bits(17..25)).unwrap()),
            0x1 => StatusCode::CommandSpecific(self.0.get_bits(17..25) as u8),
            value => panic!("Invalid status code type value: 0x{:X}", value),
        }
    }
}

impl fmt::Debug for CompletionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Completed Status")
            .field("Do Not Retry", &self.dnr())
            .field("More", &self.more())
            .field("Status Code", &self.status_code())
            .finish()
    }
}

#[repr(C)]
pub struct Completion {
    dw0: u32,
    dw1: u32,
    dw2: u32,
    dw3: u32,
}

impl Completion {
    pub fn submission_queue_id(&self) -> u16 {
        self.dw2.get_bits(16..32) as u16
    }

    pub fn command_id(&self) -> u16 {
        self.dw3.get_bits(0..16) as u16
    }

    pub fn phase_tag(&self) -> bool {
        self.dw3.get_bit(16)
    }

    pub fn status(&self) -> CompletionStatus {
        CompletionStatus(self.dw3)
    }
}

impl fmt::Debug for Completion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NVMe Command Completed")
            .field("Submission Queue ID", &self.submission_queue_id())
            .field("Command ID", &self.command_id())
            .field("Phase Tag", &self.phase_tag())
            .field("Status", &self.status())
            .finish()
    }
}
