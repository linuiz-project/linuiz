use crate::{registers::MSR, structures::apic::APIC};
use core::sync::atomic::AtomicUsize;

pub static LPU_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn get_local_data() -> Option<&'static LPU> {
    match MSR::IA32_FS_BASE.read() {
        0 => None,
        ptr => {
            let lpu = unsafe { &mut *(ptr as *mut LPU) };

            if lpu.magic == LPU::MAGIC {
                Some(lpu)
            } else {
                None
            }
        }
    }
}

pub fn local_data() -> &'static LPU {
    assert_ne!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has not been configured."
    );

    unsafe { &*(MSR::IA32_FS_BASE.read() as *const LPU) }
}

pub fn auto_init_lpu() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    unsafe {
        const LPU_STRUCTURE_SIZE: usize = crate::align_up(core::mem::size_of::<LPU>(), 0x1000);

        // Allocate space for LPU struct.
        let ptr = crate::alloc!(LPU_STRUCTURE_SIZE, 0x1000);
        core::ptr::write_bytes(ptr, 0, LPU_STRUCTURE_SIZE);
        trace!(
            "Allocating region for local CPU structure: {:?}:{}",
            ptr,
            LPU_STRUCTURE_SIZE
        );

        MSR::IA32_FS_BASE.write(ptr as u64);
        trace!(
            "IA32_FS successfully updated: 0x{:X}.",
            MSR::IA32_FS_BASE.read()
        );

        let apic = APIC::from_msr();

        *(MSR::IA32_FS_BASE.read() as *mut LPU) = LPU {
            magic: LPU::MAGIC,
            id: apic.id(),
            apic,
        };
    }

    LPU_COUNT.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
}

pub fn is_bsp() -> bool {
    use bit_field::BitField;
    MSR::IA32_APIC_BASE.read().get_bit(8)
}

pub struct LPU {
    magic: usize,
    id: u8,
    apic: APIC,
}

impl LPU {
    const MAGIC: usize = 0x132FFD5454544444;

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn apic(&'static self) -> &'static APIC {
        &self.apic
    }
}
