mod capabilities;
pub use capabilities::*;

use crate::{
    io::pci::{DeviceRegister, DeviceRegisterIterator, DeviceType, PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
};
use core::fmt;
use num_enum::TryFromPrimitive;

#[repr(usize)]
#[derive(Debug, TryFromPrimitive)]
pub enum StandardRegister {
    Register0 = 0,
    Register1 = 1,
    Register2 = 2,
    Register3 = 3,
    Register4 = 4,
    Register5 = 5,
}

impl PCIeDevice<Standard> {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        assert_eq!(
            (mmio
                .read::<u8>(crate::io::pci::HeaderOffset::HeaderType.into())
                .unwrap())
                & !(1 << 7),
            0,
            "incorrect header type for standard specification PCI device"
        );

        let mut register_num = 0;
        let mut registers = alloc::vec![None, None, None, None, None, None];
        for register in DeviceRegisterIterator::new(
            (mmio.mapped_addr() + 0x10).as_mut_ptr::<u32>(),
            Standard::REGISTER_COUNT,
        ) {
            if !register.is_unused() {
                debug!("Device Register {}: {:?}", register_num, register);

                // The address is MMIO, so is memory-mapped—thus, the page index and frame index will match.
                let frame_index = register.as_addr().page_index();
                let frame_usage = crate::align_up_div(register.memory_usage(), 0x1000);
                debug!(
                    "\tAcquiring register destination frame as MMIO: {}:{}",
                    frame_index, frame_usage
                );
                let mmio_frames = crate::memory::falloc::get()
                    .acquire_frames(
                        frame_index,
                        frame_usage,
                        crate::memory::falloc::FrameState::Reserved,
                    )
                    .expect("frames are not MMIO");
                debug!("\tAuto-mapping register destination frame.");
                let register_mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                    .expect("failed to create MMIO object")
                    .automap();

                if match register {
                    DeviceRegister::MemorySpace32(value, _) => (value & 0b1000) > 0,
                    DeviceRegister::MemorySpace64(value, _) => (value & 0b1000) > 0,
                    _ => false,
                } {
                    debug!("\tRegister is prefetchable, so enabling WRITE_THROUGH bit on page.");
                    // Optimize page attributes to enable write-through if it wasn't previously enabled.
                    for page in register_mmio.pages() {
                        use crate::memory::paging::{AttributeModify, PageAttributes};

                        crate::memory::malloc::get().set_page_attributes(
                            &page,
                            PageAttributes::WRITE_THROUGH | PageAttributes::UNCACHEABLE,
                            AttributeModify::Insert,
                        );
                    }
                }

                registers[register_num] = Some(register_mmio);
            }

            match register {
                DeviceRegister::MemorySpace64(_, _) => register_num += 2,
                _ => register_num += 1,
            }
        }

        Self {
            mmio,
            registers,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn cardbus_cis_ptr(&self) -> u32 {
        unsafe { self.mmio.read(0x28).unwrap() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2C).unwrap() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2E).unwrap() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { self.mmio.read(0x30).unwrap() }
    }

    pub fn capabilities(&self) -> CapablitiesIterator {
        CapablitiesIterator::new(&self.mmio, unsafe {
            self.mmio.read::<u8>(0x34).unwrap() & !0b11
        })
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3C).unwrap() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3D).unwrap() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.mmio.read(0x3E).unwrap() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.mmio.read(0x3F).unwrap() }
    }

    pub fn get_register(&self, register: StandardRegister) -> Option<&MMIO<Mapped>> {
        self.registers[register as usize].as_ref()
    }

    pub fn iter_registers(&self) -> core::slice::Iter<Option<MMIO<Mapped>>> {
        self.registers.iter()
    }
}

impl fmt::Debug for PCIeDevice<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debut_fmt(debug_struct);
        debug_struct
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field(
                "Expansion ROM Base Address",
                &self.expansion_rom_base_addr(),
            )
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}
