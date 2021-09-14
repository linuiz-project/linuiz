use alloc::vec::Vec;
use bit_field::BitField;
use core::{convert::TryFrom, marker::PhantomData, mem::size_of};
use libkernel::{
    addr_ty::Physical,
    io::pci::{standard::StandardRegister, PCIeDevice, Standard},
    memory::volatile::{Volatile, VolatileCell},
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
    cur_index: usize,
    max_index: usize,
    unsubmitted_entries: usize,
    doorbell: &'q VolatileCell<u32, ReadWrite>,
}

impl<'q> NVMeQueue<'q> {
    pub fn new(
        max_entries: usize,
        doorbell_stride: usize,
        doorbell: &'q VolatileCell<u32, ReadWrite>,
    ) -> Self {
        let alloc_size = max_entries * size_of::<NVMeCommand<Abort>>();

        Self {
            entries: unsafe {
                &mut *core::slice::from_raw_parts_mut(libkernel::alloc!(alloc_size), max_entries)
            },
            doorbell_stride,
            cur_index: 0,
            max_index: max_entries,
            unsubmitted_entries: 0,

            doorbell,
        }
    }

    fn increment_index(&mut self) {
        if self.cur_index == self.max_index {
            self.cur_index = 0;
        } else {
            self.cur_index += 1;
        }
    }

    pub fn next_entry<T: NVMeCommandType>(&mut self) -> Option<&mut NVMeCommand<T>> {
        if self.unsubmitted_entries < self.max_index {
            let cur_stride_index = self.cur_index * self.doorbell_stride;
            self.unsubmitted_entries += 1;
            self.increment_index();

            // transmute the &mut [u32] to an &mut NVMeCommand
            unsafe { Some(core::mem::transmute(&mut self.entries[cur_stride_index])) }
        } else {
            None
        }
    }

    pub fn flush_entries(&mut self) {
        let new_tail = (&self.entries[self.cur_index * self.doorbell_stride]) as *const u32;
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
}

#[repr(u32)]
pub enum NVMeFusedCommandInfo {
    Normal = 0b00,
    FusedFirst = 0b01,
    FusedSecond = 0b10,
}

#[repr(u32)]
pub enum NVMePSDT {
    PRP = 0b00,
    SGLBuffered = 0b01,
    SGLSegmented = 0b10,
}

#[repr(transparent)]
pub struct NVMeCommandHeader(u32);

impl NVMeCommandHeader {
    pub fn new(
        opcode: u8,
        fuse_info: NVMeFusedCommandInfo,
        psdt: NVMePSDT,
        command_identifier: u16,
    ) -> Self {
        Self(
            (opcode as u32)
                | ((fuse_info as u32) << 8)
                | ((psdt as u32) << 14)
                | ((command_identifier as u32) << 16),
        )
    }
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

#[repr(C)]
pub struct NVMeCommand<T: NVMeCommandType> {
    header: NVMeCommandHeader,
    namespace_id: u32,
    dword2: u32,
    dword3: u32,
    metadata_ptr: Address<Physical>,
    data_ptr: (Address<Physical>, Address<Physical>),
    dword10: u32,
    dword11: u32,
    dword12: u32,
    dword13: u32,
    dword14: u32,
    dword15: u32,
    phantom: PhantomData<T>,
}

impl<T: NVMeCommandType> NVMeCommand<T> {
    pub fn clear(&mut self) {
        unsafe {
            *core::mem::transmute::<&mut Self, &mut [u32; COMMAND_DWORDS]>(self) =
                [0u32; COMMAND_DWORDS];
        }
    }
}

impl NVMeCommand<Abort> {
    pub fn configure(&mut self, command_id: u16, sub_queue_id: u16) {
        self.clear();

        self.dword10 = (command_id as u32) | ((sub_queue_id as u32) << 16);
    }
}
