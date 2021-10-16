mod types;
pub use types::*;

use super::queue::QueueDomain;
use bit_field::BitField;
use core::{convert::TryFrom, fmt, marker::PhantomData, ops::Range};
use libkernel::{
    addr_ty::Physical, memory::volatile::VolatileCell, volatile_bitfield_getter,
    volatile_bitfield_getter_as, Address, ReadWrite,
};
use num_enum::TryFromPrimitive;

const COMMAND_DWORD_COUNT: usize = {
    use core::mem::size_of;

    size_of::<Command<super::queue::Admin, Abort>>() / size_of::<u32>()
};

pub type NVME_COMMAND = [u32; COMMAND_DWORD_COUNT];

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum FusedCommandInfo {
    Normal = 0b00,
    FusedFirst = 0b01,
    FusedSecond = 0b10,
}

#[repr(usize)]
pub enum DataTransfer {
    NoData = 0b00,
    HostToController = 0b01,
    ControllerToHost = 0b10,
    Bidirectional = 0b11,
}

pub enum DataPointer {
    PSDT(Address<Physical>, Address<Physical>),
    SGL(u128),
}

#[repr(C)]
pub struct Command<Q: QueueDomain, T: CommandType<Q>> {
    header: VolatileCell<u32, ReadWrite>,
    namespace_id: VolatileCell<u32, ReadWrite>,
    dword2: VolatileCell<u32, ReadWrite>,
    dword3: VolatileCell<u32, ReadWrite>,
    metadata_ptr: VolatileCell<Address<Physical>, ReadWrite>,
    data_ptr_low: VolatileCell<u64, ReadWrite>,
    data_ptr_high: VolatileCell<u64, ReadWrite>,
    dword10: VolatileCell<u32, ReadWrite>,
    dword11: VolatileCell<u32, ReadWrite>,
    dword12: VolatileCell<u32, ReadWrite>,
    dword13: VolatileCell<u32, ReadWrite>,
    dword14: VolatileCell<u32, ReadWrite>,
    dword15: VolatileCell<u32, ReadWrite>,
    phantomq: PhantomData<Q>,
    phantomt: PhantomData<T>,
}

impl<Q: QueueDomain, T: CommandType<Q>> Command<Q, T> {
    const OPCODE: Range<usize> = 0..8;
    const FUSED_INFO: Range<usize> = 8..10;
    const COMMAND_ID: Range<usize> = 16..32;
    const PSDT: Range<usize> = 14..16;
    const PRP_ENTRY_1: Range<usize> = 0..64;
    const PRP_ENTRY_2: Range<usize> = 64..128;

    pub fn clear(&mut self) {
        unsafe { core::ptr::write_bytes((&raw mut *self) as *mut u32, 0, COMMAND_DWORD_COUNT) }
    }

    volatile_bitfield_getter_as!(header, u32, u8, opcode, Self::OPCODE);

    pub fn get_fuse_info(&self) -> FusedCommandInfo {
        FusedCommandInfo::try_from(self.header.read().get_bits(Self::FUSED_INFO)).unwrap()
    }

    pub fn set_fuse_info(&mut self, fuse_info: FusedCommandInfo) {
        self.header.write(
            *self
                .header
                .read()
                .set_bits(Self::FUSED_INFO, fuse_info as u32),
        );
    }

    volatile_bitfield_getter_as!(header, u32, u16, command_id, Self::COMMAND_ID);
    volatile_bitfield_getter!(namespace_id, u32, namespace_id, 0..32);

    pub fn set_metadata_ptr(&mut self, ptr: Address<Physical>) {
        self.metadata_ptr.write(ptr);
    }

    pub fn set_data_ptr(&mut self, ptr: DataPointer) {
        match ptr {
            DataPointer::PSDT(prp_entry_1, prp_entry_2) => {
                self.data_ptr_low.write(prp_entry_1.as_usize() as u64);
                self.data_ptr_high.write(prp_entry_2.as_usize() as u64);
                self.header
                    .write(*self.header.read().set_bits(Self::PSDT, 0b00));
            }
            DataPointer::SGL(sgl) => {
                self.data_ptr_low.write(sgl as u64);
                self.data_ptr_high.write((sgl >> 64) as u64);
                // TODO segmented and buffered SGL
                self.header
                    .write(*self.header.read().set_bits(Self::PSDT, 0b01));
            }
        }
    }

    fn base_configure(&mut self, namespace_id: Option<u32>, command_id: u16) {
        self.clear();
        self.set_opcode(T::OPCODE);
        self.set_command_id(command_id);
        self.set_namespace_id(namespace_id.unwrap_or(0));
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
