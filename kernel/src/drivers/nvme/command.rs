use bit_field::BitField;
use core::{convert::TryFrom, fmt, marker::PhantomData, ops::Range};
use libkernel::{addr_ty::Physical, Address};
use num_enum::TryFromPrimitive;

const COMMAND_DWORD_COUNT: usize = {
    use core::mem::size_of;

    size_of::<Command<Abort>>() / size_of::<u32>()
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

pub trait CommandType {
    const OPCODE: u8;
}

pub enum Abort {}
impl CommandType for Abort {
    const OPCODE: u8 = 0x8;
}

pub enum CompletionCreate {}
impl CommandType for CompletionCreate {
    const OPCODE: u8 = 0x5;
}

pub enum DataPointer {
    PSDT(Address<Physical>, Address<Physical>),
    SGL(u128),
}

#[repr(C)]
pub struct Command<T: CommandType> {
    header: u32,
    namespace_id: u32,
    dword2: u32,
    dword3: u32,
    metadata_ptr: Address<Physical>,
    data_ptr: u128,
    dword10: u32,
    dword11: u32,
    dword12: u32,
    dword13: u32,
    dword14: u32,
    dword15: u32,
    phantom: PhantomData<T>,
}

impl<T: CommandType> Command<T> {
    const OPCODE: Range<usize> = 0..8;
    const FUSED_INFO: Range<usize> = 8..10;
    const COMMAND_ID: Range<usize> = 16..32;
    const PSDT: Range<usize> = 14..16;
    const PRP_ENTRY_1: Range<usize> = 0..64;
    const PRP_ENTRY_2: Range<usize> = 64..128;

    pub fn clear(&mut self) {
        unsafe {
            core::ptr::write_bytes(
                self as *mut _ as *mut u8,
                0,
                core::mem::size_of::<Command<Abort>>(),
            )
        }
    }

    pub fn get_opcode(&self) -> u8 {
        self.header.get_bits(Self::OPCODE) as u8
    }

    pub fn set_opcode(&mut self, opcode: u8) {
        self.header.set_bits(Self::OPCODE, opcode as u32);
    }

    pub fn get_fuse_info(&self) -> FusedCommandInfo {
        FusedCommandInfo::try_from(self.header.get_bits(Self::FUSED_INFO)).unwrap()
    }

    pub fn set_fuse_info(&mut self, fuse_info: FusedCommandInfo) {
        self.header.set_bits(Self::FUSED_INFO, fuse_info as u32);
    }

    pub fn get_command_id(&self) -> u16 {
        self.header.get_bits(16..32) as u16
    }

    pub fn set_command_id(&mut self, command_id: u16) {
        self.header.set_bits(16..32, command_id as u32);
    }

    pub fn set_metadata_ptr(&mut self, ptr: Address<Physical>) {
        self.metadata_ptr = ptr;
    }

    pub fn set_data_ptr(&mut self, ptr: DataPointer) {
        match ptr {
            DataPointer::PSDT(prp_entry_1, prp_entry_2) => {
                self.data_ptr
                    .set_bits(Self::PRP_ENTRY_1, prp_entry_1.as_usize() as u128);
                self.data_ptr
                    .set_bits(Self::PRP_ENTRY_2, prp_entry_2.as_usize() as u128);
                self.header.set_bits(Self::PSDT, 0b00);
            }
            DataPointer::SGL(sgl) => {
                self.data_ptr = sgl;
                // TODO segmented and buffered SGL
                self.header.set_bits(Self::PSDT, 0b01);
            }
        }
    }
}

impl Command<Abort> {
    pub fn configure(&mut self, command_id: u16, sub_queue_id: u16) {
        self.clear();

        self.dword10 = (command_id as u32) | ((sub_queue_id as u32) << 16);
    }
}

impl Command<CompletionCreate> {
    const QID: Range<usize> = 0..16;
    const QSIZE: Range<usize> = 16..32;
    const PC: usize = 0;
    const IEN: usize = 1;
    const IV: Range<usize> = 16..32;

    pub fn configure(
        &mut self,
        queue_id: u16,
        queue_size: u16,
        prp_entry: Address<Physical>,
        physically_contiguous: bool,
        interrupts_enabled: bool,
        interrupt_vector: u16,
    ) {
        self.set_data_ptr(DataPointer::PSDT(prp_entry, Address::<Physical>::zero()));
        self.dword10.set_bits(Self::QID, queue_id as u32);
        self.dword10.set_bits(Self::QSIZE, queue_size as u32);
        self.dword11.set_bit(Self::PC, physically_contiguous);
        self.dword11.set_bit(Self::IEN, interrupts_enabled);
        self.dword11.set_bits(Self::IV, interrupt_vector as u32);
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
    CommandSpecific = 0x1,
    MediaAndDataIntegrityErrors = 0x2,
    PathRelatedStatus = 0x3,
    VendorSpecific = 0x7,
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
            0 => StatusCode::Generic(GenericStatus::try_from(self.0.get_bits(17..25)).unwrap()),
            value => panic!("Invalid status code type value: {}", value),
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
