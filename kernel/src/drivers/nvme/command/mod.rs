pub mod admin;
pub mod io;

use bit_field::BitField;
use core::{convert::TryFrom, fmt, marker::PhantomData, ops::Range};
use libkernel::{
    memory::volatile::VolatileCell, volatile_bitfield_getter, volatile_bitfield_getter_as, Address,
    Physical, ReadWrite,
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

#[repr(C)]
pub struct DataPointer(u64, u64);

impl DataPointer {
    pub const fn none() -> Self {
        Self(0, 0)
    }

    pub const fn new_prp(addr0: Address<Physical>, addr1: Option<Address<Physical>>) -> Self {
        Self(addr0.as_u64(), addr1.unwrap_or(Address::zero()).as_u64())
    }
}

#[repr(C)]
// TODO make this pub(super) and remove Q
pub struct Command {
    pub opcode: u8,
    pub fuse_psdt: u8,
    pub command_id: u16,
    pub ns_id: u32,
    pub cdw2: u32,
    pub cdw3: u32,
    pub mdata_ptr: Address<Physical>,
    pub data_ptr: DataPointer,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

impl Command {
    pub const fn opcode(&self) -> u8 {
        self.opcode
    }
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
    // TODO implement CRD

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
            // TODO implement Media and Data Integrity Errors
            // TODO implement Path Related Status
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
#[derive(Clone, Copy)]
pub struct Completion {
    dw0: u32,
    dw1: u32,
    dw2: u32,
    dw3: u32,
}

impl Completion {
    pub fn get_sub_queue_id(&self) -> u16 {
        self.dw2.get_bits(16..32) as u16
    }

    pub fn get_command_id(&self) -> u16 {
        self.dw3.get_bits(0..16) as u16
    }

    pub fn get_phase_tag(&self) -> bool {
        self.dw3.get_bit(16)
    }

    pub fn get_status(&self) -> CompletionStatus {
        CompletionStatus(self.dw3)
    }
}

impl fmt::Debug for Completion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NVMe Command Completed")
            .field("Submission Queue ID", &self.get_sub_queue_id())
            .field("Command ID", &self.get_command_id())
            .field("Phase Tag", &self.get_phase_tag())
            .field("Status", &self.get_status())
            .finish()
    }
}
