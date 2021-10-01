use crate::drivers::nvme::{
    command::{Command, DataPointer},
    queue::{Admin, QueueDomain},
};
use bit_field::BitField;
use core::ops::Range;
use libkernel::{addr_ty::Physical, volatile_bitfield_getter, Address};

pub trait CommandType<Q: QueueDomain + ?Sized> {
    const OPCODE: u8;
}

pub enum Abort {}
impl CommandType<Admin> for Abort {
    const OPCODE: u8 = 0x8;
}
impl Command<Admin, Abort> {
    pub fn configure(&mut self, id: u16, abort_id: u16, abort_queue_id: u16) {
        self.base_configure(None, id);
        self.dword10
            .write((abort_id as u32) | ((abort_queue_id as u32) << 16));
    }
}

pub enum CompletionCreate {}
impl CommandType<Admin> for CompletionCreate {
    const OPCODE: u8 = 0x5;
}
impl Command<Admin, CompletionCreate> {
    const QID: Range<usize> = 0..16;
    const QSIZE: Range<usize> = 16..32;
    const PC: usize = 0;
    const IEN: usize = 1;
    const IV: Range<usize> = 16..32;

    pub fn configure(
        &mut self,
        id: u16,
        queue_id: u16,
        queue_size: u16,
        prp_entry: Address<Physical>,
        physically_contiguous: bool,
        interrupts_enabled: bool,
        interrupt_vector: u16,
    ) {
        self.base_configure(None, id);
        self.set_data_ptr(DataPointer::PSDT(prp_entry, Address::<Physical>::zero()));
        let (mut dword10, mut dword11) = (self.dword10.read(), self.dword11.read());

        dword10.set_bits(Self::QID, queue_id as u32);
        dword10.set_bits(Self::QSIZE, queue_size as u32);
        dword11.set_bit(Self::PC, physically_contiguous);
        dword11.set_bit(Self::IEN, interrupts_enabled);
        dword11.set_bits(Self::IV, interrupt_vector as u32);

        self.dword10.write(dword10);
        self.dword11.write(dword11);
    }
}

#[repr(u32)]
pub enum DeviceSelfTestCode {
    Short = 0x1,
    Extended = 0x2,
    Abort = 0xF,
}

pub enum DeviceSelfTest {}
impl CommandType<Admin> for DeviceSelfTest {
    const OPCODE: u8 = 0x14;
}
impl Command<Admin, DeviceSelfTest> {
    const CODE: Range<usize> = 0..4;

    volatile_bitfield_getter!(dword10, u32, code, Self::CODE);

    pub fn configure(&mut self, id: u16, code: DeviceSelfTestCode) {
        self.base_configure(None, id);
        self.set_code(code as u32);
    }
}
