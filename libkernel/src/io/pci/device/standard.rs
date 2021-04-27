use crate::{
    addr_ty::{Physical, Virtual},
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
    Address,
};
use core::fmt;

pub trait BaseAddressRegisterType {
    fn base_address<T>(raw: u32) -> *const T;
}

pub enum MemorySpace {}
impl BaseAddressRegisterType for MemorySpace {
    fn base_address<T>(raw: u32) -> *const T {
        (raw & !0b1111) as *const _
    }
}

pub enum IOSpace {}
impl BaseAddressRegisterType for IOSpace {
    fn base_address<T>(raw: u32) -> *const T {
        (raw & !0b11) as *const _
    }
}

#[derive(Debug)]
pub enum BaseAddressRegisterVariant<'a> {
    MemorySpace(&'a BaseAddressRegister<MemorySpace>),
    IOSpace(&'a BaseAddressRegister<IOSpace>),
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySpaceAddressType {
    Bit32 = 0b00,
    Bit64 = 0b10,
    Reserved = 0b01,
    Invalid = 0b11,
}

#[repr(transparent)]
pub struct BaseAddressRegister<V: BaseAddressRegisterType> {
    value: u32,
    phantom: core::marker::PhantomData<V>,
}

impl<V: BaseAddressRegisterType> BaseAddressRegister<V> {
    pub fn base_address(&self) -> Address<Virtual> {
        Address::<Virtual>::from_ptr(V::base_address::<u8>(self.value))
    }
}

impl BaseAddressRegister<MemorySpace> {
    pub fn prefetchable(&self) -> bool {
        (self.value & (1 << 3)) > 0
    }

    pub fn address_type(&self) -> MemorySpaceAddressType {
        match (self.value >> 1) & 0b11 {
            0 => MemorySpaceAddressType::Bit32,
            1 => MemorySpaceAddressType::Reserved,
            2 => MemorySpaceAddressType::Bit64,
            _ => MemorySpaceAddressType::Invalid,
        }
    }
}

impl fmt::Debug for BaseAddressRegister<MemorySpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BaseAddressRegister<MemorySpace>")
            .field("Base Address", &self.base_address())
            .field("Prefetchable", &self.prefetchable())
            .field("Type", &self.address_type())
            .finish()
    }
}

impl fmt::Debug for BaseAddressRegister<IOSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BaseAddressRegister<IOSpace>")
            .field(&self.base_address())
            .finish()
    }
}

impl PCIeDevice<Standard> {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        const ONCE_CELL_EMPTY: core::lazy::OnceCell<MMIO<Mapped>> = core::lazy::OnceCell::new();

        assert_eq!(
            *mmio
                .read::<u32>(crate::io::pci::PCIHeaderOffset::HeaderType.into())
                .unwrap(),
            0,
            "incorrect header type for standard specification PCI device"
        );

        let this = Self {
            mmio,
            bar_mmios: [ONCE_CELL_EMPTY; 10],
            phantom: core::marker::PhantomData,
        };

        for register in 0..=5 {
            let register_raw = *this
                .mmio
                .read::<u32>(0x10 + (register * core::mem::size_of::<u32>()))
                .unwrap();

            if register_raw == 0x0 {
                continue;
            } else {
                let is_memory_space = (register_raw & 0b1) == 0;

                let addr = Address::<Physical>::new({
                    if is_memory_space {
                        register_raw & !0b1111
                    } else {
                        register_raw & 0b11
                    }
                } as usize);

                let mmio_frames = crate::memory::falloc::get()
                    .acquire_frames(
                        addr.frame_index(),
                        1,
                        crate::memory::falloc::FrameState::MMIO,
                    )
                    .expect("frames are not MMIO");
                let register_mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                    .expect("failed to create MMIO object")
                    .automap();

                if is_memory_space && ((register_raw & 0b1000) > 0) {
                    use crate::memory::paging::{PageAttributeModifyMode, PageAttributes};

                    crate::memory::malloc::get().modify_page_attributes(
                        &crate::memory::Page::from_addr(register_mmio.mapped_addr()),
                        PageAttributes::WRITE_THROUGH,
                        PageAttributeModifyMode::Insert,
                    )
                }

                this.bar_mmios[register]
                    .set(register_mmio)
                    .expect("already configured MMIO register");
            }
        }

        this
    }

    pub fn register0(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[0].get_mut()
    }

    pub fn register1(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[1].get_mut()
    }

    pub fn register2(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[2].get_mut()
    }

    pub fn register3(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[3].get_mut()
    }

    pub fn register4(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[4].get_mut()
    }

    pub fn register5(&'mmio self) -> Option<&'mmio mut MMIO<Mapped>> {
        self.bar_mmios[5].get_mut()
    }

    pub fn cardbus_cis_ptr(&self) -> &u32 {
        unsafe { *self.mmio.read(0x28).unwrap() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x2C).unwrap() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { *self.mmio.read(0x2E).unwrap() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { *self.mmio.read(0x30).unwrap() }
    }

    pub fn capabilities_ptr(&self) -> u8 {
        unsafe { *self.mmio.read::<u8>(0x34).unwrap() & !0b11 }
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { *self.mmio.read(0x3C).unwrap() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { *self.mmio.read(0x3D).unwrap() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { *self.mmio.read(0x3E).unwrap() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { *self.mmio.read(0x3F).unwrap() }
    }
}

impl fmt::Debug for PCIeDevice<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PCIe Device (Standard)")
            .field("Base Address Register 0", &self.register0())
            .field("Base Address Register 1", &self.register1())
            .field("Base Address Register 2", &self.register2())
            .field("Base Address Register 3", &self.register3())
            .field("Base Address Register 4", &self.register4())
            .field("Base Address Register 5", &self.register5())
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field(
                "Expansion ROM Base Address",
                &self.expansion_rom_base_addr(),
            )
            .field("Capabilities Pointer", &self.capabilities_ptr())
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}
