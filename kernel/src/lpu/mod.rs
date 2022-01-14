mod interrupt_controller;

pub use interrupt_controller::*;

use crate::clock::AtomicClock;
use core::sync::atomic::AtomicUsize;
use libstd::registers::MSR;

pub static LPU_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    let apic_id = {
        use bit_field::BitField;
        libstd::instructions::cpuid::exec(0x1, 0x0)
            .unwrap()
            .ebx()
            .get_bits(24..) as u8
    };
    debug!("Configuring LPU state: {}.", apic_id);

    unsafe {
        let ptr: *mut LPU = libstd::alloc!(1)
            .expect("Unrecoverable error in LPU creation")
            .into_parts()
            .0;

        // Write the LPU structure into memory.
        ptr.write_volatile(LPU {
            magic: LPU::MAGIC,
            apic_id,
            clock: AtomicClock::new(),
            int_ctrl: InterruptController::create(),
        });

        LPU_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        MSR::IA32_FS_BASE.write(ptr as u64);
        trace!("IA32_FS updated: 0x{:X}.", MSR::IA32_FS_BASE.read());

        libstd::instructions::interrupts::enable();
    }
}

pub fn is_bsp() -> bool {
    use bit_field::BitField;
    MSR::IA32_APIC_BASE.read().get_bit(8)
}

pub fn try_get() -> Option<&'static LPU> {
    unsafe { (MSR::IA32_FS_BASE.read() as *const LPU).as_ref() }
        .filter(|lpu| lpu.magic == LPU::MAGIC)
}

pub struct LPU {
    magic: usize,
    apic_id: u8,
    clock: AtomicClock,
    int_ctrl: InterruptController,
}

impl LPU {
    const MAGIC: usize = 0x132FFD5454544444;

    pub fn id(&self) -> u8 {
        self.apic_id
    }

    pub fn clock(&self) -> &AtomicClock {
        &self.clock
    }

    pub fn int_ctrl(&self) -> &InterruptController {
        &self.int_ctrl
    }
}
