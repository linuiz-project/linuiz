pub mod standard;

use crate::{
    memory::{
        volatile::{Volatile, VolatileCell},
        MMIO,
    },
    volatile_bitfield_getter, volatile_bitfield_getter_ro, ReadWrite,
};
use alloc::vec::Vec;
use bitflags::bitflags;
use core::{convert::TryFrom, fmt, marker::PhantomData};
use num_enum::TryFromPrimitive;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum HeaderOffset {
    VendorID = 0x0,
    DeviceID = 0x2,
    Command = 0x4,
    Status = 0x6,
    RevisionID = 0x8,
    ProgramInterface = 0x9,
    Subclass = 0xA,
    Class = 0xB,
    CacheLineSize = 0xC,
    LatencyTimer = 0xD,
    HeaderType = 0xE,
    BuiltInSelfTest = 0xF,
}

impl Into<u32> for HeaderOffset {
    fn into(self) -> u32 {
        self as u32
    }
}

impl Into<usize> for HeaderOffset {
    fn into(self) -> usize {
        self as usize
    }
}

pub struct CommandRegister {
    reg: VolatileCell<u32, ReadWrite>,
}

impl CommandRegister {
    volatile_bitfield_getter_ro!(reg, io_space, 0);
    volatile_bitfield_getter_ro!(reg, memory_space, 1);
    volatile_bitfield_getter!(reg, bus_master, 2);
    volatile_bitfield_getter_ro!(reg, special_cycle, 3);
    volatile_bitfield_getter_ro!(reg, memory_w_and_i, 4);
    volatile_bitfield_getter_ro!(reg, vga_palette_snoop, 5);
    volatile_bitfield_getter!(reg, parity_error, 6);
    volatile_bitfield_getter_ro!(reg, idsel_stepwait_cycle_ctrl, 7);
    volatile_bitfield_getter!(reg, serr_num, 8);
    volatile_bitfield_getter_ro!(reg, fast_b2b_transactions, 9);
    volatile_bitfield_getter!(reg, interrupt_disable, 10);
}

impl Volatile for CommandRegister {}

impl fmt::Debug for CommandRegister {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Command Register")
            .field("IO Space", &self.get_io_space())
            .field("Memory Space", &self.get_memory_space())
            .field("Bus Master", &self.get_bus_master())
            .field("Special Cycle", &self.get_special_cycle())
            .field("Memory Write & Invalidate", &self.get_memory_w_and_i())
            .field("VGA Palette Snoop", &self.get_vga_palette_snoop())
            .field("Parity Error", &self.get_parity_error())
            .field(
                "IDSEL Stepping/Wait Cycle Control",
                &self.get_idsel_stepwait_cycle_ctrl(),
            )
            .field("SERR#", &self.get_serr_num())
            .field(
                "Fast Back-to-Back Transactions",
                &self.get_fast_b2b_transactions(),
            )
            .field("Interrupt Disable", &self.get_interrupt_disable())
            .finish()
    }
}

#[repr(u16)]
#[derive(Debug, TryFromPrimitive)]
pub enum DEVSELTiming {
    Fast = 0b00,
    Medium = 0b01,
    Slow = 0b10,
}

bitflags! {
    pub struct StatusRegister: u16 {
        const INTERRUPT_STATUS = 1 << 3;
        const CAPABILITIES = 1 << 4;
        // * Not applicable to PCIe.
        const CAPABILITITY_66MHZ = 1 << 5;
        // * Not applicable to PCIe.
        const FAST_BACK2BACK_CAPABLE = 1 << 7;
        const MASTER_DATA_PARITY_ERROR = 1 << 8;
        // * Not applicable to PCIe.
        const DEVSEL_TIMING = 3 << 9;
        const SIGNALED_TARGET_ABORT = 1 << 11;
        const RECEIVED_TARGET_ABORT = 1 << 12;
        const RECEIVED_MASTER_ABORT =  1 << 13;
        const SIGNALED_SYSTEM_ERROR = 1 << 14;
        const DETECTED_PARITY_ERROR = 1 << 15;
    }
}

impl StatusRegister {
    pub fn devsel_timing(&self) -> DEVSELTiming {
        DEVSELTiming::try_from((self.bits() >> 9) & 0b11).unwrap()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Unclassified = 0x0,
    MassStorageController = 0x1,
    NetworkController = 0x2,
    DisplayController = 0x3,
    MultimediaController = 0x4,
    MemoryController = 0x5,
    Bridge = 0x6,
    CommunicationController = 0x7,
    GenericSystemPeripheral = 0x8,
    InputDeviceController = 0x9,
    DockingStation = 0xA,
    Processor = 0xB,
    SerialBusController = 0xC,
    WirelessController = 0xD,
    IntelligentController = 0xE,
    SatelliteCommunicationsController = 0xF,
    EncryptionController = 0x10,
    SignalProcessingController = 0x11,
    ProcessingAccelerators = 0x12,
    NonEssentialInstrumentation = 0x13,
    Coprocessor = 0x40,
    Unassigned = 0xFF,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BuiltinSelfTest {
    data: u8,
}

impl BuiltinSelfTest {
    pub fn capable(&self) -> bool {
        (self.data & (1 << 7)) > 0
    }

    pub fn start(&self) -> bool {
        (self.data & (1 << 6)) > 0
    }

    pub fn completion_code(&self) -> u8 {
        self.data & 0b111
    }
}

impl fmt::Debug for BuiltinSelfTest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BIST")
            .field("Capable", &self.capable())
            .field("Start", &self.start())
            .field("Completion Code", &self.completion_code())
            .finish()
    }
}

pub trait DeviceType {
    const REGISTER_COUNT: usize;
}

pub enum Standard {}
impl DeviceType for Standard {
    const REGISTER_COUNT: usize = 6;
}

pub enum PCI2PCI {}
impl DeviceType for PCI2PCI {
    const REGISTER_COUNT: usize = 2;
}

pub enum PCI2CardBus {}
impl DeviceType for PCI2CardBus {
    const REGISTER_COUNT: usize = 8;
}

#[derive(Debug)]
pub enum DeviceVariant {
    Standard(PCIeDevice<Standard>),
    PCI2PCI(PCIeDevice<PCI2PCI>),
    PCI2CardBus(PCIeDevice<PCI2CardBus>),
}

pub struct PCIeDevice<T: DeviceType> {
    mmio: MMIO,
    registers: Vec<Option<MMIO>>,
    phantom: PhantomData<T>,
}

// TODO move frame_manager and page_manager into libkernel ... again

pub fn new_device(mmio: MMIO) -> DeviceVariant {
    let type_malfunc = unsafe {
        mmio.read::<u8>(HeaderOffset::HeaderType.into())
            .assume_init()
    };

    // mask off the multifunction bit
    match type_malfunc & !(1 << 7) {
        0x0 => DeviceVariant::Standard(unsafe { PCIeDevice::<Standard>::new(mmio) }),
        0x1 => DeviceVariant::PCI2PCI(PCIeDevice {
            mmio,
            registers: Vec::new(),
            phantom: PhantomData,
        }),
        0x2 => DeviceVariant::PCI2CardBus(PCIeDevice::<PCI2CardBus> {
            mmio,
            registers: Vec::new(),
            phantom: PhantomData,
        }),
        invalid_type => {
            panic!("Header type is invalid (must be 0..=2): {}", invalid_type,)
        }
    }
}

impl<T: DeviceType> PCIeDevice<T> {
    pub fn vendor_id(&self) -> u16 {
        unsafe { self.mmio.read(HeaderOffset::VendorID.into()).assume_init() }
    }

    pub fn device_id(&self) -> u16 {
        unsafe { self.mmio.read(HeaderOffset::DeviceID.into()).assume_init() }
    }

    pub fn command(&self) -> &CommandRegister {
        unsafe { self.mmio.borrow(HeaderOffset::Command.into()) }
    }

    pub fn status(&self) -> StatusRegister {
        unsafe { self.mmio.read(HeaderOffset::Status.into()).assume_init() }
    }

    pub fn revision_id(&self) -> u8 {
        unsafe {
            self.mmio
                .read(HeaderOffset::RevisionID.into())
                .assume_init()
        }
    }

    pub fn program_interface(&self) -> u8 {
        unsafe {
            self.mmio
                .read(HeaderOffset::ProgramInterface.into())
                .assume_init()
        }
    }

    pub fn subclass(&self) -> u8 {
        unsafe { self.mmio.read(HeaderOffset::Subclass.into()).assume_init() }
    }

    pub fn class(&self) -> DeviceClass {
        unsafe { self.mmio.read(HeaderOffset::Class.into()).assume_init() }
    }

    pub fn cache_line_size(&self) -> u8 {
        unsafe {
            self.mmio
                .read(HeaderOffset::CacheLineSize.into())
                .assume_init()
        }
    }

    pub fn latency_timer(&self) -> u8 {
        unsafe {
            self.mmio
                .read(HeaderOffset::LatencyTimer.into())
                .assume_init()
        }
    }

    pub fn header_type(&self) -> u8 {
        unsafe {
            self.mmio
                .read::<u8>(HeaderOffset::HeaderType.into())
                .assume_init()
                & !(1 << 7)
        }
    }

    pub fn multi_function(&self) -> bool {
        unsafe {
            (self
                .mmio
                .read::<u8>(HeaderOffset::BuiltInSelfTest.into())
                .assume_init()
                & (1 << 7))
                > 0
        }
    }

    pub fn builtin_self_test(&self) -> BuiltinSelfTest {
        unsafe {
            self.mmio
                .read(HeaderOffset::BuiltInSelfTest.into())
                .assume_init()
        }
    }

    pub fn generic_debut_fmt(&self, debug_struct: &mut fmt::DebugStruct) {
        debug_struct
            .field("Vendor ID", &self.vendor_id())
            .field("Device ID", &self.device_id())
            .field("Command", &self.command())
            .field("Status", &self.status())
            .field("Revision ID", &self.revision_id())
            .field("Class Code", &self.class())
            .field("Sub Class", &self.subclass())
            .field("Cache Line Size", &self.cache_line_size())
            .field("Master Latency Timer", &self.latency_timer())
            .field("Header Type", &self.header_type())
            .field("Built-In Self Test", &self.builtin_self_test());
    }
}

#[derive(Debug)]
pub enum DeviceRegister {
    MemorySpace32(u32, usize),
    MemorySpace64(u64, usize),
    IOSpace(u32, usize),
    None,
}

impl DeviceRegister {
    pub fn is_unused(&self) -> bool {
        use bit_field::BitField;

        match self {
            DeviceRegister::MemorySpace32(value, _) => value.get_bits(4..32) == 0,
            DeviceRegister::MemorySpace64(value, _) => value.get_bits(4..64) == 0,
            DeviceRegister::IOSpace(value, _) => value.get_bits(2..32) == 0,
            DeviceRegister::None => true,
        }
    }

    pub fn as_addr(&self) -> crate::Address<crate::Virtual> {
        use crate::{Address, Virtual};

        Address::<Virtual>::new(match self {
            DeviceRegister::MemorySpace32(value, _) => (value & !0b1111) as usize,
            DeviceRegister::MemorySpace64(value, _) => (value & !0b1111) as usize,
            DeviceRegister::IOSpace(value, _) => (value & !0b11) as usize,
            DeviceRegister::None => 0,
        })
    }

    pub fn memory_usage(&self) -> usize {
        match self {
            DeviceRegister::MemorySpace32(_, mem_usage) => *mem_usage,
            DeviceRegister::MemorySpace64(_, mem_usage) => *mem_usage,
            DeviceRegister::IOSpace(_, mem_usage) => *mem_usage,
            DeviceRegister::None => 0,
        }
    }
}

pub struct DeviceRegisterIterator {
    base: *mut u32,
    max_base: *mut u32,
}

impl DeviceRegisterIterator {
    unsafe fn new(base: *mut u32, register_count: usize) -> Self {
        Self {
            base,
            max_base: base.add(register_count),
        }
    }
}

impl Iterator for DeviceRegisterIterator {
    type Item = DeviceRegister;

    fn next(&mut self) -> Option<Self::Item> {
        if self.base < self.max_base {
            unsafe {
                let register_raw = self.base.read_volatile();

                let register = {
                    use bit_field::BitField;

                    if register_raw == 0 {
                        DeviceRegister::None
                    } else if register_raw.get_bit(0) {
                        self.base.write_volatile(u32::MAX);
                        let mem_usage = !(self.base.read_volatile() & !0b11) + 1;
                        self.base.write_volatile(register_raw);

                        DeviceRegister::IOSpace(register_raw, mem_usage as usize)
                    } else {
                        match register_raw.get_bits(1..3) {
                            // REMARK:
                            //  This little dance with the reads & writes is just fucking magic?
                            //  Who comes up with this shit?
                            0b00 => {
                                // Write all 1's to register.
                                self.base.write_volatile(u32::MAX);
                                // Record memory usage by masking address bits, NOT'ing, and adding one.
                                let mem_usage = !(self.base.read_volatile() & !0b1111) + 1;
                                // Write original value back into register.
                                self.base.write_volatile(register_raw);

                                DeviceRegister::MemorySpace32(register_raw, mem_usage as usize)
                            }
                            // And because of MMIO volatility, it's even dumber for 64-bit registers
                            0b10 => {
                                let base_next = self.base.add(1);
                                // Record value of next register to restore later.
                                let register_raw_next = base_next.read_volatile();

                                // Write all 1's into double-wide register.
                                self.base.write(u32::MAX);
                                base_next.write(u32::MAX);

                                // Record raw values of double-wide register.
                                let register_raw_u64 = (self.base.read_volatile() as u64)
                                    | ((base_next.read_volatile() as u64) << 32);

                                // Record memory usage of double-wide register.
                                let mem_usage = !(register_raw_u64 & !0b1111) + 1;

                                // Write old raw values back into double-wide register.
                                self.base.write_volatile(register_raw);
                                base_next.write_volatile(register_raw_next);

                                DeviceRegister::MemorySpace64(
                                    (register_raw as u64) | ((register_raw_next as u64) << 32),
                                    mem_usage as usize,
                                )
                            }
                            _ => panic!("invalid register type: 0b{:b}", register_raw),
                        }
                    }
                };

                match register {
                    DeviceRegister::MemorySpace64(_, _) => self.base = self.base.add(2),
                    _ => self.base = self.base.add(1),
                }

                Some(register)
            }
        } else {
            None
        }
    }
}

impl fmt::Debug for PCIeDevice<PCI2PCI> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}

impl fmt::Debug for PCIeDevice<PCI2CardBus> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ss").finish()
    }
}
