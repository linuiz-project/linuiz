use spin::MutexGuard;

use crate::{
    addr_ty::{Physical, Virtual},
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
    Address,
};
use core::fmt;

impl PCIeDevice<Standard> {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        assert_eq!(
            (*mmio
                .read::<u8>(crate::io::pci::PCIHeaderOffset::HeaderType.into())
                .unwrap())
                & !(1 << 7),
            0,
            "incorrect header type for standard specification PCI device"
        );

        let bar_mmios = [PCIeDevice::<Standard>::EMPTY_REG; 10];

        for register_num in 0..=5 {
            let register_raw = *mmio
                .read::<u32>(0x10 + (register_num * core::mem::size_of::<u32>()))
                .unwrap();

            if register_raw > 0x0 {
                let is_memory_space = (register_raw & 0b1) > 0;
                let addr = Address::<Physical>::new({
                    if is_memory_space {
                        register_raw & !0b1111
                    } else {
                        register_raw & !0b11
                    }
                } as usize);

                trace!(
                    "Device Register {}:\n Raw 0b{:b}\n Canonical: {:?}",
                    register_num,
                    register_raw,
                    addr
                );

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

                    // optimize page attributes to enable write-through if it wasn't previously enabled
                    crate::memory::malloc::get().modify_page_attributes(
                        &crate::memory::Page::from_addr(register_mmio.mapped_addr()),
                        PageAttributes::WRITE_THROUGH,
                        PageAttributeModifyMode::Insert,
                    )
                }

                bar_mmios[register_num]
                    .set(spin::Mutex::new(register_mmio))
                    .expect("already configured MMIO register");
            }
        }

        Self {
            mmio,
            bar_mmios,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn reg0(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[0].get().map(|mutex| mutex.lock())
    }

    pub fn reg1(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[1].get().map(|mutex| mutex.lock())
    }

    pub fn reg2(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[2].get().map(|mutex| mutex.lock())
    }

    pub fn reg3(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[3].get().map(|mutex| mutex.lock())
    }

    pub fn reg4(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[4].get().map(|mutex| mutex.lock())
    }

    pub fn reg5(&self) -> Option<MutexGuard<MMIO<Mapped>>> {
        self.bar_mmios[5].get().map(|mutex| mutex.lock())
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
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debut_fmt(debug_struct);
        debug_struct
            .field("Base Address Register 0", &self.reg0())
            .field("Base Address Register 1", &self.reg1())
            .field("Base Address Register 2", &self.reg2())
            .field("Base Address Register 3", &self.reg3())
            .field("Base Address Register 4", &self.reg4())
            .field("Base Address Register 5", &self.reg5())
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
