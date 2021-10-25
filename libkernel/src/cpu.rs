use crate::{registers::MSR, structures::apic::APIC};

pub fn lpu() -> &'static CPU {
    assert_ne!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has not been configured."
    );

    unsafe { &*(MSR::IA32_FS_BASE.read() as *const CPU) }
}

pub fn auto_init_lpu() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    unsafe {
        const LPU_STRUCTURE_SIZE: usize = 0x1000;

        // Allocate space for LPU struct.
        let ptr = crate::alloc!(LPU_STRUCTURE_SIZE, 0x1000);
        core::ptr::write_bytes(ptr, 0, LPU_STRUCTURE_SIZE);
        debug!(
            "Allocating region for local CPU structure: {:?}:{}",
            ptr, LPU_STRUCTURE_SIZE
        );

        MSR::IA32_FS_BASE.write(ptr as u64);
        debug!(
            "IA32_FS successfully updated: 0x{:X}.",
            MSR::IA32_FS_BASE.read()
        );

        let lpu = &mut *(MSR::IA32_FS_BASE.read() as *mut CPU);

        let frames = {
            use crate::memory::falloc;

            falloc::get()
                .acquire_frame(
                    (MSR::IA32_APIC_BASE.read() & 0x3FFFFF000) as usize,
                    falloc::FrameState::Reserved,
                )
                .unwrap()
                .into_iter()
        };
        let mapped_mmio = crate::memory::mmio::unmapped_mmio(frames)
            .unwrap()
            .automap();

        debug!(
            "LPU's LAPIC mapped to virtual address: {:?}",
            mapped_mmio.mapped_addr()
        );

        lpu.lapic = APIC::new(mapped_mmio);
    }

    info!(".");
    let apic = lpu().apic();
    info!(".");
    let id = apic.id();
    info!(".");
    debug!("Configured local procesing unit {}.", lpu().apic().id());
}

pub struct CPU {
    lapic: APIC,
}

impl CPU {
    pub fn apic(&'static self) -> &'static APIC {
        &self.lapic
    }
}
