use crate::{registers::MSR, structures::apic::APIC};
use core::sync::atomic::AtomicUsize;

pub static LPU_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn try_get() -> Option<&'static LPU> {
    match MSR::IA32_FS_BASE.read() {
        0 => None,
        ptr => {
            let lpu = unsafe { (ptr as *const LPU).as_ref().unwrap() };

            if lpu.magic == LPU::MAGIC {
                Some(lpu)
            } else {
                None
            }
        }
    }
}

pub fn get() -> &'static LPU {
    assert_ne!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has not been configured."
    );

    unsafe { (MSR::IA32_FS_BASE.read() as *const LPU).as_ref().unwrap() }
}

pub fn init() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    unsafe {
        const LPU_STRUCTURE_SIZE: usize = crate::align_up(core::mem::size_of::<LPU>(), 0x1000);

        let (ptr, len): (*mut u8, usize) =
            crate::alloc!(LPU_STRUCTURE_SIZE, core::num::NonZeroUsize::new(0x1000))
                .expect("Unrecoverable error in LPU creation")
                .into_parts();
        core::ptr::write_bytes(ptr, 0, len);
        trace!("Allocated memory for LPU structure: {:?}:{}", ptr, len);

        MSR::IA32_FS_BASE.write(ptr as u64);
        trace!("IA32_FS updated: 0x{:X}.", MSR::IA32_FS_BASE.read());

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
