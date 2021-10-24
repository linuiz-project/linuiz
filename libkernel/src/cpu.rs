use crate::{registers::MSR, structures::apic::APIC};

pub fn lpu() -> &'static CPU {
    assert_ne!(
        MSR::IA32_FS.read(),
        0,
        "IA32_FS MSR has not been configured."
    );

    unsafe { &*(MSR::IA32_FS.read() as *const CPU) }
}

pub fn auto_init_lpu() {
    assert_eq!(
        MSR::IA32_FS.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    unsafe {
        // Allocate space for LPU struct.
        let ptr = crate::alloc!(core::mem::size_of::<CPU>(), 0x1000);
        core::ptr::write_bytes(ptr, 0, 0x1000);
        debug!(
            "Allocating region for local CPU structure: {:?}:{}",
            ptr, 0x1000
        );
        MSR::IA32_FS.write(ptr as u64);

        let lpu = &mut *(MSR::IA32_FS.read() as *mut CPU);
        // Configure LPU's local APIC.
        lpu.lapic = {
            use crate::memory::falloc;
            APIC::new(
                crate::memory::mmio::unmapped_mmio(
                    falloc::get()
                        .acquire_frame(
                            ((MSR::IA32_APIC_BASE.read() & !0xFFF) >> 12) as usize,
                            falloc::FrameState::Reserved,
                        )
                        .unwrap()
                        .into_iter(),
                )
                .unwrap()
                .automap(),
            )
        };
    }

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
