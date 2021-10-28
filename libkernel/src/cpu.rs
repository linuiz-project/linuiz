use crate::{registers::MSR, structures::apic::APIC};
use core::sync::atomic::AtomicUsize;

pub static LPU_COUNT: AtomicUsize = AtomicUsize::new(0);

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

        lpu.lapic = APIC::from_ia32_apic_base();
    }

    LPU_COUNT.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
}

pub struct CPU {
    lapic: APIC,
}

impl CPU {
    pub fn apic(&'static self) -> &'static APIC {
        &self.lapic
    }
}
