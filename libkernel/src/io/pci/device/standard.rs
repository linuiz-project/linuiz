use crate::{
    io::pci::{
        PCIeDevice, PCIeDeviceRegister, PCIeDeviceRegisterIterator, PCIeDeviceType, Standard,
    },
    memory::mmio::{Mapped, MMIO},
};
use core::fmt;

impl PCIeDevice<Standard> {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        assert_eq!(
            (mmio
                .read::<u8>(crate::io::pci::PCIHeaderOffset::HeaderType.into())
                .unwrap()
                .read())
                & !(1 << 7),
            0,
            "incorrect header type for standard specification PCI device"
        );

        for (register_num, register_value) in PCIeDeviceRegisterIterator::new(
            mmio.mapped_addr().as_ptr::<u32>().add(0x4),
            Standard::REGISTER_COUNT,
        )
        .enumerate()
        {
            let addr = register_value.as_addr();
            if !addr.is_null() {
                debug!("Device Register {}: {:?}", register_num, register_value);

                // The address is MMIO, so is memory-mapped—thus,
                //  the page index and frame index will match.
                let frame_index = addr.page_index();
                debug!(
                    "\tAcquiring register destination frame as MMIO: {}",
                    frame_index
                );
                let mmio_frames = crate::memory::falloc::get()
                    .acquire_frames(frame_index, 1, crate::memory::falloc::FrameState::MMIO)
                    .expect("frames are not MMIO");
                debug!("\tAuto-mapping register destination frame.");
                let register_mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                    .expect("failed to create MMIO object")
                    .automap();

                debug!("\tDetermining register prefetchability.");
                if match register_value {
                    PCIeDeviceRegister::MemorySpace32(value) => (value & 0b1000) > 0,
                    PCIeDeviceRegister::MemorySpace64(value) => (value & 0b1000) > 0,
                    _ => false,
                } {
                    use crate::memory::paging::{PageAttributeModifyMode, PageAttributes};

                    debug!("\tRegister is prefetchable, so enabling WRITE_THROUGH bit on page.");
                    // optimize page attributes to enable write-through if it wasn't previously enabled
                    crate::memory::malloc::get().modify_page_attributes(
                        &crate::memory::Page::from_addr(register_mmio.mapped_addr()),
                        PageAttributes::WRITE_THROUGH,
                        PageAttributeModifyMode::Insert,
                    )
                }
            }
        }

        Self {
            mmio,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn cardbus_cis_ptr(&self) -> u32 {
        unsafe { self.mmio.read(0x28).unwrap().read() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2C).unwrap().read() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2E).unwrap().read() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { self.mmio.read(0x30).unwrap().read() }
    }

    pub fn capabilities_ptr(&self) -> u8 {
        unsafe { self.mmio.read::<u8>(0x34).unwrap().read() & !0b11 }
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3C).unwrap().read() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3D).unwrap().read() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.mmio.read(0x3E).unwrap().read() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.mmio.read(0x3F).unwrap().read() }
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
            .field("Capabilities Pointer", &self.capabilities_ptr())
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}
