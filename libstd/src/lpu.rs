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

pub fn init() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    unsafe {
        use bit_field::BitField;

        let cpuid = crate::instructions::cpuid(0x1, 0x0).unwrap();
        let apic_id = cpuid.ebx().get_bits(24..) as u8;
        let htt_count = cpuid.ebx().get_bits(16..24) as u8;
        let apic = APIC::from_msr();

        let ptr: *mut LPU = crate::memory::malloc::try_get()
            .unwrap()
            .alloc(core::mem::size_of::<LPU>(), None)
            .expect("Unrecoverable error in LPU creation")
            .into_parts()
            .0 as *mut _;

        // Write the LPU structure into memory.
        ptr.write_volatile(LPU {
            magic: LPU::MAGIC,
            apic_id,
            htt_count,
            apic,
        });

        MSR::IA32_FS_BASE.write(ptr as u64);
        debug!("IA32_FS updated: 0x{:X}.", MSR::IA32_FS_BASE.read());
    }

    debug!(
        "LPU state structure finalized: {}",
        try_get().expect("Unexpected error occured attempting to access newly-configured LPU state structure").id()
    );

    LPU_COUNT.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
}

pub fn is_bsp() -> bool {
    use bit_field::BitField;
    MSR::IA32_APIC_BASE.read().get_bit(8)
}

pub struct LPU {
    magic: usize,
    apic_id: u8,
    htt_count: u8,
    apic: APIC,
}

impl LPU {
    const MAGIC: usize = 0x132FFD5454544444;

    pub fn id(&self) -> u8 {
        self.apic_id
    }

    pub fn apic(&'static self) -> &'static APIC {
        &self.apic
    }
}
