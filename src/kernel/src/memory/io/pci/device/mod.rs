pub mod standard;

use core::{fmt, marker::PhantomData};
use libkernel::{Address, Physical};

use crate::num::LittleEndianU32;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Command(u32);

// TODO impl command bits
// impl CommandRegister {
//     volatile_bitfield_getter_ro!(reg, io_space, 0);
//     volatile_bitfield_getter_ro!(reg, memory_space, 1);
//     volatile_bitfield_getter!(reg, bus_master, 2);
//     volatile_bitfield_getter_ro!(reg, special_cycle, 3);
//     volatile_bitfield_getter_ro!(reg, memory_w_and_i, 4);
//     volatile_bitfield_getter_ro!(reg, vga_palette_snoop, 5);
//     volatile_bitfield_getter!(reg, parity_error, 6);
//     volatile_bitfield_getter_ro!(reg, idsel_stepwait_cycle_ctrl, 7);
//     volatile_bitfield_getter!(reg, serr_num, 8);
//     volatile_bitfield_getter_ro!(reg, fast_b2b_transactions, 9);
//     volatile_bitfield_getter!(reg, interrupt_disable, 10);
// }

// impl Volatile for CommandRegister {}

// impl fmt::Debug for CommandRegister {
//     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
//         formatter
//             .debug_struct("Command Register")
//             .field("IO Space", &self.get_io_space())
//             .field("Memory Space", &self.get_memory_space())
//             .field("Bus Master", &self.get_bus_master())
//             .field("Special Cycle", &self.get_special_cycle())
//             .field("Memory Write & Invalidate", &self.get_memory_w_and_i())
//             .field("VGA Palette Snoop", &self.get_vga_palette_snoop())
//             .field("Parity Error", &self.get_parity_error())
//             .field("IDSEL Stepping/Wait Cycle Control", &self.get_idsel_stepwait_cycle_ctrl())
//             .field("SERR#", &self.get_serr_num())
//             .field("Fast Back-to-Back Transactions", &self.get_fast_b2b_transactions())
//             .field("Interrupt Disable", &self.get_interrupt_disable())
//             .finish()
//     }
// }

// #[repr(u16)]
// #[derive(Debug, TryFromPrimitive)]
// pub enum DEVSELTiming {
//     Fast = 0b00,
//     Medium = 0b01,
//     Slow = 0b10,
// }

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct Status : u32 {
        const INTERRUPT_STATUS = 1 << 19;
        const CAPABILITIES = 1 << 20;
        /// * Not applicable to PCIe.
        const CAPABILITITY_66MHZ = 1 << 21;
        /// * Not applicable to PCIe.
        const FAST_BACK2BACK_CAPABLE = 1 << 23;
        const MASTER_DATA_PARITY_ERROR = 1 << 24;
        /// * Not applicable to PCIe.
        const DEVSEL_TIMING = 3 << 25;
        const SIGNALED_TARGET_ABORT = 1 << 27;
        const RECEIVED_TARGET_ABORT = 1 << 28;
        const RECEIVED_MASTER_ABORT =  1 << 29;
        const SIGNALED_SYSTEM_ERROR = 1 << 30;
        const DETECTED_PARITY_ERROR = 1 << 31;
    }
}

// impl StatusRegister {
//     pub fn devsel_timing(&self) -> DEVSELTiming {
//         DEVSELTiming::try_from((self.bits() >> 9) & 0b11).unwrap()
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Unclassified(Unclassified),
    MassStorageController(MassStorageController),
    Other(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unclassified {
    NonVgaCompatible,
    VgaCompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassStorageController {
    Scsi,
    Ide,
    Floppy,
    Ipi,
    Raid,
    AtaSingleStep,
    AtaContinuous,
    SataVendorSpecific,
    SataAhci,
    Sas,
    Other,
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
    Standard(Device<Standard>),
    PCI2PCI(Device<PCI2PCI>),
    PCI2CardBus(Device<PCI2CardBus>),
}

pub struct Device<T: DeviceType> {
    base_ptr: *mut LittleEndianU32,
    phantom: PhantomData<T>,
}

// SAFETY: PCI MMIO (and so, the pointers used for it) utilize the global HHDM, and so can be sent between threads.
unsafe impl<T: DeviceType> Send for Device<T> {}

/// SAFETY: Caller must ensure that the provided base pointer is a valid (and mapped) PCI MMIO header base.
pub unsafe fn new_device(base_ptr: *mut LittleEndianU32) -> DeviceVariant {
    let header_type = (unsafe { base_ptr.add(0x3).read_volatile().get() } >> 16) & 0x3F;

    // mask off the multifunction bit
    match header_type {
        0x0 => DeviceVariant::Standard(Device::<Standard> { base_ptr, phantom: PhantomData }),
        0x1 => DeviceVariant::PCI2PCI(Device { base_ptr, phantom: PhantomData }),
        0x2 => DeviceVariant::PCI2CardBus(Device::<PCI2CardBus> { base_ptr, phantom: PhantomData }),
        header_type => {
            panic!("Header type is invalid (must be 0..=2): {}", header_type,)
        }
    }
}

impl<T: DeviceType> Device<T> {
    pub fn get_vendor_id(&self) -> u16 {
        unsafe { (self.base_ptr.add(0x0).read_volatile().get() >> 0) as u16 }
    }

    pub fn get_device_id(&self) -> u16 {
        unsafe { (self.base_ptr.add(0x0).read_volatile().get() >> 16) as u16 }
    }

    pub fn get_command(&self) -> Command {
        Command(unsafe { (self.base_ptr.add(0x1).read_volatile().get() >> 0) & 0xFFFF })
    }

    pub fn set_command(&self, value: Command) {
        unsafe {
            self.base_ptr.add(0x1).write_volatile(LittleEndianU32::new(self.get_status().bits() | (value.0 as u32)))
        }
    }

    pub fn get_status(&self) -> Status {
        Status::from_bits_truncate(unsafe { self.base_ptr.add(0x1).read_volatile().get() })
    }

    pub fn get_revision_id(&self) -> u8 {
        unsafe { (self.base_ptr.add(0x2).read_volatile().get() >> 0) as u8 }
    }

    pub fn get_class(&self) -> Class {
        // Match format is:
        //  0x  00      | 00        | 00
        //      Class   | Subclass  | Program interface

        match unsafe { self.base_ptr.add(0x2).read_volatile().get() >> 8 } {
            // Unclassified
            0x00_00_00 => Class::Unclassified(Unclassified::NonVgaCompatible),
            0x00_01_00 => Class::Unclassified(Unclassified::VgaCompatible),

            // Mass storage
            0x01_00_00 => Class::MassStorageController(MassStorageController::Scsi),
            0x01_01_00..0x01_01_FF => Class::MassStorageController(MassStorageController::Ide),
            0x01_02_00 => Class::MassStorageController(MassStorageController::Floppy),
            0x01_03_00 => Class::MassStorageController(MassStorageController::Ipi),
            0x01_04_00 => Class::MassStorageController(MassStorageController::Raid),
            0x01_05_20 => Class::MassStorageController(MassStorageController::AtaSingleStep),
            0x01_05_30 => Class::MassStorageController(MassStorageController::AtaContinuous),
            0x01_06_00 => Class::MassStorageController(MassStorageController::SataVendorSpecific),
            0x01_06_01 => Class::MassStorageController(MassStorageController::SataAhci),
            0x01_07_00 => Class::MassStorageController(MassStorageController::Sas),
            0x01_80_00 => Class::MassStorageController(MassStorageController::Other),

            class => Class::Other((class >> 16) as u8, (class >> 8) as u8, (class >> 0) as u8),
        }
    }

    pub fn get_cache_line_size(&self) -> u8 {
        unsafe { (self.base_ptr.add(0x3).read_volatile().get() >> 0) as u8 }
    }

    pub fn get_latency_timer(&self) -> u8 {
        unsafe { (self.base_ptr.add(0x3).read_volatile().get() >> 8) as u8 }
    }

    pub fn get_header_type(&self) -> u8 {
        unsafe { ((self.base_ptr.add(0x3).read_volatile().get() >> 16) & 0x3F) as u8 }
    }

    pub fn get_multi_function(&self) -> bool {
        (unsafe { self.base_ptr.add(0x3).read_volatile().get() } & (1 << 23)) > 0
    }

    pub fn get_bar(&self, index: usize) -> Option<BAR> {
        use bit_field::BitField;

        assert!(index < T::REGISTER_COUNT);

        // SAFETY: PCI spec indicates the address space always contains BARs.
        let base_bar_ptr = unsafe { self.base_ptr.add(0x4) };

        // We need to check if this BAR is the upper-half of a 64-bit BAR
        // SAFETY: The lower and upper bound of the index are already checked, so we know the following BAR pointer is valid.
        if index > 0 && unsafe { base_bar_ptr.add(index - 1).read_volatile() }.get().get_bits(0..3).eq(&0b100) {
            None
        } else {
            // SAFETY: See above about PCI spec.
            let bar_ptr = unsafe { base_bar_ptr.add(index) };
            // SAFETY: See above about PCI spec.
            let bar_data = unsafe { bar_ptr.read_volatile() }.get();

            // Check whether BAAR is IO space
            if bar_data.get_bit(0) {
                Some(BAR::IOSpace { address: bar_data & !0b11, size: 0 })
            } else {
                match bar_data.get_bits(1..3) {
                    0b00 => Some({
                        // SAFETY: See above about PCI spec.
                        let size = unsafe {
                            bar_ptr.write_volatile(LittleEndianU32::new(u32::MAX));
                            let bar_size = !(bar_ptr.read_volatile().get() & !0b1111) + 1;

                            bar_ptr.write_volatile(LittleEndianU32::new(bar_data));

                            bar_size
                        };

                        BAR::MemorySpace32 {
                            address: (bar_data & !0b1111) as usize as *mut u32,
                            size,
                            prefetch: bar_data.get_bit(3),
                        }
                    }),

                    0b10 => Some({
                        // SAFETY: See above about PCI spec.
                        let bar_high_ptr = unsafe { bar_ptr.add(0x1) };
                        // SAFETY: See above about PCI spec.
                        let bar_high_data = unsafe { bar_high_ptr.read_volatile() }.get();

                        // SAFETY: See above about PCI spec.
                        let size = unsafe {
                            bar_ptr.write_volatile(LittleEndianU32::new(u32::MAX));
                            bar_ptr.add(1).write_volatile(LittleEndianU32::new(u32::MAX));
                            let bar_values =
                                ((bar_ptr.read_volatile().get() as u64) << 32) | (bar_ptr.read_volatile().get() as u64);
                            let bar_size = !(bar_values & !0b1111) + 1;

                            bar_ptr.write_volatile(LittleEndianU32::new(bar_data));
                            bar_high_ptr.write_volatile(LittleEndianU32::new(bar_high_data));

                            bar_size
                        };

                        BAR::MemorySpace64 {
                            address: (((bar_high_data as u64) << 32) | ((bar_data as u64) & !0b1111)) as usize
                                as *mut u64,
                            size,
                            prefetch: bar_data.get_bit(3),
                        }
                    }),

                    type_bits => {
                        warn!("Unsupported `type` bits for PCI BAR: {:b}", type_bits);
                        None
                    }
                }
            }
        }
    }

    pub fn generic_debut_fmt(&self, debug_struct: &mut fmt::DebugStruct) {
        debug_struct
            .field("ID", &format_args!("{:4X}:{:4X}", self.get_vendor_id(), self.get_device_id()))
            .field("Command", &format_args!("{:?}", self.get_command()))
            .field("Status", &self.get_status())
            .field("Revision ID", &self.get_revision_id())
            .field("Class", &format_args!("{:?}", self.get_class()))
            .field("Cache Line Size", &self.get_cache_line_size())
            .field("Master Latency Timer", &self.get_latency_timer())
            .field("Header Type", &self.get_header_type());
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BAR {
    MemorySpace32 { address: *mut u32, size: u32, prefetch: bool },
    MemorySpace64 { address: *mut u64, size: u64, prefetch: bool },
    IOSpace { address: u32, size: u32 },
}

impl BAR {
    pub fn is_unused(&self) -> bool {
        use bit_field::BitField;

        match self {
            BAR::MemorySpace32 { address, size: _, prefetch: _ } => address.is_null(),
            BAR::MemorySpace64 { address, size: _, prefetch: _ } => address.is_null(),
            BAR::IOSpace { address, size: _ } => address.get_bits(2..32) == 0,
        }
    }

    pub const fn get_size(&self) -> usize {
        match self {
            BAR::MemorySpace32 { address: _, size, prefetch: _ } => *size as usize,
            BAR::MemorySpace64 { address: _, size, prefetch: _ } => *size as usize,
            BAR::IOSpace { address: _, size } => *size as usize,
        }
    }

    pub fn get_address(&self) -> Address<Physical> {
        match self {
            BAR::MemorySpace32 { address, size: _, prefetch: _ } => Address::<Physical>::new_truncate(*address as u64),
            BAR::MemorySpace64 { address, size: _, prefetch: _ } => Address::<Physical>::new_truncate(*address as u64),
            BAR::IOSpace { address, size: _ } => Address::<Physical>::new_truncate(*address as u64),
        }
    }
}

// pub struct DeviceRegisterIterator {
//     base: *mut u32,
//     max_base: *mut u32,
// }

// impl DeviceRegisterIterator {
//     unsafe fn new(base: *mut u32, register_count: usize) -> Self {
//         Self { base, max_base: base.add(register_count) }
//     }
// }

// impl Iterator for DeviceRegisterIterator {
//     type Item = BAR;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.base < self.max_base {
//             unsafe {
//                 let register_raw = self.base.read_volatile();

//                 let register = {
//                     use bit_field::BitField;

//                     if register_raw == 0 {
//                         BAR::None
//                     } else if register_raw.get_bit(0) {
//                         self.base.write_volatile(u32::MAX);
//                         let mem_usage = !(self.base.read_volatile() & !0b11) + 1;
//                         self.base.write_volatile(register_raw);

//                         BAR::IOSpace(register_raw, mem_usage as usize)
//                     } else {
//                         match register_raw.get_bits(1..3) {
//                             // REMARK:
//                             //  This little dance with the reads & writes is just fucking magic?
//                             //  Who comes up with this shit?
//                             0b00 => {
//                                 // Write all 1's to register.
//                                 self.base.write_volatile(u32::MAX);
//                                 // Record memory usage by masking address bits, NOT'ing, and adding one.
//                                 let mem_usage = !(self.base.read_volatile() & !0b1111) + 1;
//                                 // Write original value back into register.
//                                 self.base.write_volatile(register_raw);

//                                 BAR::MemorySpace32(register_raw, mem_usage as usize)
//                             }
//                             // And because of MMIO volatility, it's even dumber for 64-bit registers
//                             0b10 => {
//                                 let base_next = self.base.add(1);
//                                 // Record value of next register to restore later.
//                                 let register_raw_next = base_next.read_volatile();

//                                 // Write all 1's into double-wide register.
//                                 self.base.write(u32::MAX);
//                                 base_next.write(u32::MAX);

//                                 // Record raw values of double-wide register.
//                                 let register_raw_u64 =
//                                     (self.base.read_volatile() as u64) | ((base_next.read_volatile() as u64) << 32);

//                                 // Record memory usage of double-wide register.
//                                 let mem_usage = !(register_raw_u64 & !0b1111) + 1;

//                                 // Write old raw values back into double-wide register.
//                                 self.base.write_volatile(register_raw);
//                                 base_next.write_volatile(register_raw_next);

//                                 BAR::MemorySpace64(
//                                     (register_raw as u64) | ((register_raw_next as u64) << 32),
//                                     mem_usage as usize,
//                                 )
//                             }
//                             _ => panic!("invalid register type: 0b{:b}", register_raw),
//                         }
//                     }
//                 };

//                 match register {
//                     BAR::MemorySpace64(_, _) => self.base = self.base.add(2),
//                     _ => self.base = self.base.add(1),
//                 }

//                 Some(register)
//             }
//         } else {
//             None
//         }
//     }
// }

impl core::fmt::Debug for Device<PCI2PCI> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Not Implemented").finish()
    }
}

impl core::fmt::Debug for Device<PCI2CardBus> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Not Implemented").finish()
    }
}
