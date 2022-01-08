use crate::{
    addr_ty::Virtual,
    io::pci::{standard::StandardRegister, PCIeDevice, Standard},
    memory::volatile::{Volatile, VolatileCell},
    volatile_bitfield_getter, Address, ReadOnly, ReadWrite,
};
use bit_field::BitField;
use core::{convert::TryFrom, fmt};

#[repr(C)]
pub struct MessageControl {
    reg0: VolatileCell<u32, ReadWrite>,
}

impl MessageControl {
    pub fn get_table_len(&self) -> usize {
        self.reg0.read().get_bits(16..27) as usize
    }

    volatile_bitfield_getter!(reg0, force_mask, 30);
    volatile_bitfield_getter!(reg0, enable, 31);
}

impl fmt::Debug for MessageControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Message Control")
            .field("Enabled", &self.get_enable())
            .field("Force Mask", &self.get_force_mask())
            .field("Table Size", &self.get_table_len())
            .finish()
    }
}

#[repr(C)]
pub struct Message {
    addr_low: VolatileCell<u32, ReadWrite>,
    addr_high: VolatileCell<u32, ReadWrite>,
    data: VolatileCell<u32, ReadWrite>,
    mask: VolatileCell<u32, ReadWrite>,
}

impl Message {
    pub fn get_masked(&self) -> bool {
        self.mask.read().get_bit(0)
    }

    pub fn set_masked(&self, masked: bool) {
        self.mask.write(*self.mask.read().set_bit(0, masked));
    }

    pub fn get_addr(&self) -> Address<Virtual> {
        let addr_low = self.addr_low.read() as usize;
        assert!(
            (addr_low & 0b11) == 0,
            "Software has failed to maintain DWORD alignment for message address."
        );
        let addr_high = (self.addr_high.read() as usize) << 32;

        Address::<Virtual>::new(addr_high | addr_low)
    }

    pub fn set_addr(&self, addr: Address<Virtual>) {
        assert!(
            self.get_masked(),
            "Cannot modify message state when unmasked."
        );
        assert!(
            addr.is_aligned(0b100),
            "Address must be aligned to a DWORD boundary."
        );

        let addr_usize = addr.as_usize();
        self.addr_low.write(addr_usize as u32);
        self.addr_high.write((addr_usize >> 32) as u32);
    }

    pub fn get_data(&self) -> u32 {
        self.data.read()
    }

    pub fn set_data(&self, value: u32) {
        assert!(
            self.get_masked(),
            "Cannot modify message state when unmasked."
        );
        self.data.write(value);
    }
}

impl Volatile for Message {}

impl fmt::Debug for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Message Table Entry")
            .field("Masked", &self.get_masked())
            .field("Address", &self.get_addr())
            .field("Data", &self.get_data())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingBit {
    Unset,
    Set,
}

impl From<usize> for PendingBit {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Unset,
            1 => Self::Set,
            value => panic!("Invalid pending bit value: {}", value),
        }
    }
}

impl Into<usize> for PendingBit {
    fn into(self) -> usize {
        match self {
            PendingBit::Unset => 0,
            PendingBit::Set => 1,
        }
    }
}

impl crate::BitValue for PendingBit {
    const BIT_WIDTH: usize = 0x1;
    const MASK: usize = 0x1;
}

#[repr(C)]
struct Data {
    reg0: VolatileCell<u32, ReadWrite>,
    reg1: VolatileCell<u32, ReadOnly>,
    reg2: VolatileCell<u32, ReadOnly>,
}

impl Data {
    pub fn get_enable(&self) -> bool {
        self.reg0.read().get_bit(31)
    }

    pub fn set_enable(&self, enable: bool) {
        self.reg0.write(*self.reg0.read().set_bit(31, enable));
    }

    pub fn get_function_mask(&self) -> bool {
        self.reg0.read().get_bit(30)
    }

    pub fn set_function_mask(&self, mask_all: bool) {
        self.reg0.write(*self.reg0.read().set_bit(30, mask_all));
    }

    fn get_table_len(&self) -> usize {
        // Field is encoded as N-1, so add one to get N (table length).
        (self.reg0.read().get_bits(16..27) as usize) + 1
    }

    fn get_table_info(&self) -> (StandardRegister, usize) {
        let reg1 = self.reg1.read();

        (
            StandardRegister::try_from(reg1.get_bits(0..3) as usize).unwrap(),
            (reg1 & !0b111) as usize,
        )
    }

    fn get_pending_info(&self) -> (StandardRegister, usize) {
        let reg2 = self.reg2.read();

        (
            StandardRegister::try_from(reg2.get_bits(0..3) as usize).unwrap(),
            (reg2 & !0b111) as usize,
        )
    }
}

pub struct MSIX<'dev> {
    data: &'dev Data,
    messages: &'dev [Message],
}

impl<'dev> MSIX<'dev> {
    pub(in crate::io::pci::device::standard) fn try_new(
        device: &'dev PCIeDevice<Standard>,
    ) -> Option<Self> {
        device
            .capabilities()
            .find(|(_, capability)| *capability == super::Capablities::MSIX)
            .map(|(addr, _)| {
                let data = unsafe { addr.as_ptr::<Data>().as_ref() }.unwrap();
                let (msg_register, msg_offset) = data.get_table_info();

                Self {
                    data,
                    messages: device
                        .get_register(msg_register)
                        .map(|mmio| unsafe { mmio.slice(msg_offset, data.get_table_len()) })
                        .expect(
                            "Device does not have requisite BARs to construct MSIX capability.",
                        ),
                }
            })
    }

    pub fn get_enable(&self) -> bool {
        self.data.get_enable()
    }

    pub fn set_enable(&self, enable: bool) {
        self.data.set_enable(enable);
    }

    pub fn get_function_mask(&self) -> bool {
        self.data.get_function_mask()
    }

    pub fn set_function_mask(&self, mask_all: bool) {
        self.data.set_function_mask(mask_all);
    }

    pub fn iter_messages(&self) -> core::slice::Iter<Message> {
        self.messages.iter()
    }
}

impl fmt::Debug for MSIX<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MSI-X")
            .field("Enabled", &self.get_enable())
            .field("Function Mask", &self.get_function_mask())
            .field("Messages", &self.messages)
            .finish()
    }
}
