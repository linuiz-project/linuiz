use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug)]
pub enum AcpiAddress {
    Port(u32),
    Other(GenericAddress),
}

/// The address space where the data structure or register exists.
#[repr(u8)]
#[derive(Debug, FromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum AddressLocation {
    /// The 64-bit physical memory address (relative to the processor) of the
    /// register. 32-bit platforms must have the high DWORD set to 0.
    SystemMemory = 0,

    /// The 64-bit I/O address (relative to the processor) of the register.
    /// 32-bit platforms must have the high DWORD set to 0.
    SystemIO = 1,

    /// PCI Configuration space addresses must be confined to devices on PCI
    /// Segment Group 0, bus 0. This restriction exists to accommodate access to
    /// fixed hardware prior to PCI bus enumeration. The format of addresses are
    /// defined as follows:
    ///
    /// Word Location Description
    /// Highest Word Reserved (must be 0)
    /// - PCI Device number on bus 0
    /// - PCI Function number
    ///
    /// Longest Word Offset in the configuration space header
    /// For example: Offset 0x23 of Function 2, Device 7, Bus 0, Segment 0:
    /// `0x0000000700020023`.
    PciConfiguration = 2,

    /// Used to locate a MMIO register on a PCI device BAR space. PCI
    /// Configuration space addresses must be confined to devices on a host bus,
    /// i.e any bus returned by a _BBN object. This restriction exists to
    /// accommodate access to fixed hardware prior to PCI bus enumeration. The
    /// format of the Address field for this type of address is:
    /// - 0..37:  Offset from BAR in `u32`s
    /// - 27..40: BAR index#
    /// - 40..43: PCI Function
    /// - 43..48: PCI Device
    /// - 48..56: PCI Bus
    /// - 56..64: PCI Segment
    PciBar = 6,

    EmbeddedController = 3,
    SMBus = 4,
    SystemCMOS = 5,
    Ipmi = 7,
    GeneralPurposeIO = 8,
    GenericSerialBus = 9,
    PlatformCommunicationsChannel = 10,
    FunctionalFixedHardware = 127,

    #[default]
    Unknown,
}

#[repr(u8)]
#[derive(Debug, FromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum AccessSize {
    Bytes1 = 1,
    Bytes2 = 2,
    Bytes4 = 3,
    Bytes8 = 4,

    #[default]
    Unknown,
}

pub struct GenericAddress {
    location: AddressLocation,
    bit_width: u32,
    bit_offset: u32,
    access_size: AccessSize,
    address: u64,
}

impl GenericAddress {
    pub fn new(
        location: AddressLocation,
        bit_width: u32,
        bit_offset: u32,
        access_size: AccessSize,
        address: u64,
    ) -> Self {
        Self {
            location,
            bit_width,
            bit_offset,
            access_size,
            address,
        }
    }

    pub fn address_location(&self) -> AddressLocation {
        self.location
    }

    pub fn bit_width(&self) -> u32 {
        self.bit_width
    }

    pub fn bit_offset(&self) -> u32 {
        self.bit_offset
    }

    pub fn access_size(&self) -> AccessSize {
        self.access_size
    }

    pub fn address(&self) -> u64 {
        self.address
    }
}

impl core::fmt::Debug for GenericAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Generic Address")
            .field("Address Location", &self.address_location())
            .field("Bit Width", &self.bit_width())
            .field("Bit Offset", &self.bit_offset())
            .field("Access Size", &self.access_size())
            .field("Address", &self.address())
            .finish()
    }
}
