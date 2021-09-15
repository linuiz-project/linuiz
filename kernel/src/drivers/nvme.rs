use alloc::vec::Vec;
use bit_field::BitField;
use core::{convert::TryFrom, marker::PhantomData, mem::size_of, ops::Range};
use libkernel::{
    addr_ty::{Physical, Virtual},
    io::pci::{standard::StandardRegister, PCIeDevice, Standard},
    memory::volatile::VolatileCell,
    volatile_bitfield_getter_ro, Address, ReadOnly, ReadWrite,
};
use num_enum::TryFromPrimitive;

const COMMAND_DWORDS: usize = size_of::<NVMeCommand<Abort>>() / size_of::<u32>();

#[repr(u64)]
#[derive(Debug, TryFromPrimitive)]
pub enum NVMECPS {
    NotReported = 0b00,
    ControllerScope = 0b01,
    DomainScope = 0b10,
    NVMSubsystemScope = 0b11,
}

#[repr(transparent)]
pub struct NVMeCapabilities {
    value: VolatileCell<u64, ReadOnly>,
}

/// NVME Capabilities Register
/// An explanation of these values can be found at:
///     https://nvmexpress.org/wp-content/uploads/NVMe-NVM-Express-2.0a-2021.07.26-Ratified.pdf
///     Figure 36
impl NVMeCapabilities {
    volatile_bitfield_getter_ro!(value, u64, mqes, 0..16);
    volatile_bitfield_getter_ro!(value, cqr, 16);
    volatile_bitfield_getter_ro!(value, u64, ams, 17..19);
    // 19..24 reserved
    volatile_bitfield_getter_ro!(value, u64, to, 24..32);
    volatile_bitfield_getter_ro!(value, u64, dstrd, 32..36);
    volatile_bitfield_getter_ro!(value, nssrs, 36);
    volatile_bitfield_getter_ro!(value, u64, css, 37..45);
    volatile_bitfield_getter_ro!(value, bps, 45);

    pub fn get_cps(&self) -> NVMECPS {
        NVMECPS::try_from(self.value.read().get_bits(46..48)).unwrap()
    }

    volatile_bitfield_getter_ro!(value, u64, mpsmin, 48..52);
    volatile_bitfield_getter_ro!(value, u64, mpsmax, 52..56);
    volatile_bitfield_getter_ro!(value, pmrs, 56);
    volatile_bitfield_getter_ro!(value, cmbs, 57);
    volatile_bitfield_getter_ro!(value, nsss, 58);
    volatile_bitfield_getter_ro!(value, crwms, 59);
    volatile_bitfield_getter_ro!(value, crims, 60);
    // 60..64 reserved
}

impl libkernel::memory::volatile::Volatile for NVMeCapabilities {}

impl core::fmt::Debug for NVMeCapabilities {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("NVME Capabilities")
            .field("MQES", &self.get_mqes())
            .field("CQR", &self.get_cqr())
            .field("AMS", &self.get_ams())
            .field("TO", &self.get_to())
            .field("DSTRD", &self.get_dstrd())
            .field("NSSRS", &self.get_nssrs())
            .field("CSS", &self.get_css())
            .field("BPS", &self.get_bps())
            .field("CPS", &self.get_cps())
            .field("MPSMIN", &self.get_mpsmin())
            .field("MPSMAX", &self.get_mpsmax())
            .field("PMRS", &self.get_pmrs())
            .field("NSSS", &self.get_nsss())
            .field("CRWMS", &self.get_crwms())
            .field("CRIMS", &self.get_crims())
            .finish()
    }
}

pub struct NVMeQueue<'q> {
    entries: &'q mut [[u32; COMMAND_DWORDS]],
    doorbell_stride: usize,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
    cur_index: usize,
    unsubmitted_entries: usize,
}

impl<'q> NVMeQueue<'q> {
    pub fn new(
        max_entries: usize,
        doorbell_stride: usize,
        doorbell: &'q VolatileCell<u32, ReadWrite>,
    ) -> Self {
        Self {
            entries: libkernel::slice_mut!([u32; COMMAND_DWORDS], max_entries),
            doorbell_stride,
            doorbell,
            cur_index: 0,
            unsubmitted_entries: 0,
        }
    }

    fn increment_index(&mut self) {
        if self.cur_index >= self.entries.len() {
            self.cur_index = 0;
        } else {
            self.cur_index += self.doorbell_stride;
        }
    }

    pub fn next_entry<T: NVMeCommandType>(&mut self) -> Option<&mut NVMeCommand<T>> {
        if self.unsubmitted_entries < (self.entries.len() / self.doorbell_stride) {
            let cached_index = self.cur_index;
            self.unsubmitted_entries += 1;
            self.increment_index();

            // transmute the &mut [u32] to an &mut NVMeCommand
            unsafe { Some(core::mem::transmute(&mut self.entries[cached_index])) }
        } else {
            None
        }
    }

    pub fn flush_entries(&mut self) {
        let new_tail = (&raw const self.entries[self.cur_index]) as *const u32;
        self.doorbell.write(new_tail as u32);
        self.unsubmitted_entries = 0;
    }
}

impl Drop for NVMeQueue<'_> {
    fn drop(&mut self) {
        unsafe {
            libkernel::memory::malloc::get().dealloc(
                self.entries.as_mut_ptr() as *mut u8,
                core::alloc::Layout::from_size_align_unchecked(
                    self.entries.len() * core::mem::size_of::<u32>(),
                    self.doorbell_stride,
                ),
            )
        }
    }
}

pub struct NVMe<'dev> {
    device: &'dev PCIeDevice<Standard>,
    pub admin_sub: NVMeQueue<'dev>,
    pub admin_com: Vec<u32>,
}

impl<'dev> NVMe<'dev> {
    fn doorbell_offset(&self, queue_id: usize, is_start: bool, is_completion: bool) -> usize {
        let base_offset = 0x1000;
        let queue_offset = 2 * (queue_id + (is_completion as usize));
        let doorbell_stride = 4 << self.capabilities().get_dstrd();

        base_offset + (queue_offset * doorbell_stride)
    }

    pub fn new(device: &'dev PCIeDevice<Standard>) -> Self {
        let reg0 = device.get_register(StandardRegister::Register0).unwrap();
        let capabilities = unsafe { reg0.borrow::<NVMeCapabilities>(0).unwrap() };
        let admin_com = alloc::vec![0u32; 50];
        unsafe {
            reg0.write(0x1000 + (4 << capabilities.get_dstrd()), admin_com.as_ptr())
                .unwrap()
        };

        Self {
            device,
            admin_sub: NVMeQueue::new(
                capabilities.get_mqes() as usize,
                (capabilities.get_dstrd() + 1) as usize,
                unsafe {
                    &*(reg0.mapped_addr() + 0x1000).as_mut_ptr::<VolatileCell<u32, ReadWrite>>()
                },
            ),
            admin_com,
        }
    }

    pub fn capabilities(&self) -> &NVMeCapabilities {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .borrow(0)
                .unwrap()
        }
    }

    /// The admin submission & completion queue sizes (in entries).
    ///     - 1st `u16`: submission queue
    ///     - 2nd `u16`: completion queue
    pub fn admin_queue_attribs(&self) -> (u16, u16) {
        let admin_queue_attribs = unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read::<u32>(0x24)
        }
        .unwrap();

        let submission_size = admin_queue_attribs.get_bits(0..12) as u16;
        let completion_size = admin_queue_attribs.get_bits(16..28) as u16;

        assert!(
            (2..=4096).contains(&submission_size),
            "maximum admin submission queue size is 2..=4096"
        );
        assert!(
            (2..=4096).contains(&completion_size),
            "maximum admin completion queue size is 2..=4096"
        );

        (submission_size, completion_size)
    }

    // pub fn get_admin_submission_queue_addr(&mut self) -> Address<Physical> {
    //     let queue_phys_addr = self.device.get_register(StandardRegister::Register0).unwrap().read::<u64>()
    //     Address::<Physical>::new()
    // }
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum NVMeFusedCommandInfo {
    Normal = 0b00,
    FusedFirst = 0b01,
    FusedSecond = 0b10,
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum NVMePSDT {
    PRP = 0b00,
    SGLBuffered = 0b01,
    SGLSegmented = 0b10,
}

pub trait NVMeCommandType {
    const OPCODE: u8;
}

pub enum Abort {}
impl NVMeCommandType for Abort {
    const OPCODE: u8 = 0;
}

pub enum CompletionCreate {}
impl NVMeCommandType for CompletionCreate {
    const OPCODE: u8 = 0;
}

pub enum NVMeDataPointer {
    PSDT(Address<Physical>, Address<Physical>),
    SGL(u128),
}

#[repr(C)]
pub struct NVMeCommand<T: NVMeCommandType> {
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

impl<T: NVMeCommandType> NVMeCommand<T> {
    const OPCODE: Range<usize> = 0..8;
    const FUSED_INFO: Range<usize> = 8..10;
    const COMMAND_ID: Range<usize> = 16..32;
    const PSDT: Range<usize> = 14..16;
    const PRP_ENTRY_1: Range<usize> = 0..64;
    const PRP_ENTRY_2: Range<usize> = 64..128;

    pub fn clear(&mut self) {
        unsafe {
            *core::mem::transmute::<&mut Self, &mut [u32; COMMAND_DWORDS]>(self) =
                [0u32; COMMAND_DWORDS];
        }
    }

    pub fn get_opcode(&self) -> u16 {
        self.header.get_bits(Self::OPCODE) as u16
    }

    pub fn set_opcode(&mut self, opcode: u16) {
        self.header.set_bits(Self::OPCODE, opcode as u32);
    }

    pub fn get_fuse_info(&self) -> NVMeFusedCommandInfo {
        NVMeFusedCommandInfo::try_from(self.header.get_bits(Self::FUSED_INFO)).unwrap()
    }

    pub fn set_fuse_info(&mut self, fuse_info: NVMeFusedCommandInfo) {
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

    pub fn set_data_ptr(&mut self, ptr: NVMeDataPointer) {
        match ptr {
            NVMeDataPointer::PSDT(prp_entry_1, prp_entry_2) => {
                self.data_ptr
                    .set_bits(Self::PRP_ENTRY_1, prp_entry_1.as_usize() as u128);
                self.data_ptr
                    .set_bits(Self::PRP_ENTRY_2, prp_entry_2.as_usize() as u128);
                self.header.set_bits(Self::PSDT, 0b00);
            }
            NVMeDataPointer::SGL(sgl) => {
                self.data_ptr = sgl;
                // TODO segmented and buffered SGL
                self.header.set_bits(Self::PSDT, 0b01);
            }
        }
    }
}

impl NVMeCommand<Abort> {
    pub fn configure(&mut self, command_id: u16, sub_queue_id: u16) {
        self.clear();

        self.dword10 = (command_id as u32) | ((sub_queue_id as u32) << 16);
    }
}

impl NVMeCommand<CompletionCreate> {
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
        self.set_data_ptr(NVMeDataPointer::PSDT(
            prp_entry,
            Address::<Physical>::zero(),
        ));
        self.dword10.set_bits(Self::QID, queue_id as u32);
        self.dword10.set_bits(Self::QSIZE, queue_size as u32);
        self.dword11.set_bit(Self::PC, physically_contiguous);
        self.dword11.set_bit(Self::IEN, interrupts_enabled);
        self.dword11.set_bits(Self::IV, interrupt_vector as u32);
    }
}
