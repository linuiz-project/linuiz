mod int_ctrl;

use bit_field::BitField;
pub use int_ctrl::*;

use crate::clock::AtomicClock;
use core::sync::atomic::AtomicUsize;
use libstd::registers::MSR;

pub static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    INIT_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    let apic_id = {
        libstd::instructions::cpuid::exec(0x1, 0x0)
            .unwrap()
            .ebx()
            .get_bits(24..) as u64
    };
    debug!("Configuring local state: {}.", apic_id);

    unsafe {
        let lpu_ptr = libstd::memory::malloc::try_get()
            .unwrap()
            .alloc(
                core::mem::size_of::<LocalState>(),
                // Align to 0x1000 to accomodate some state bits.
                core::num::NonZeroUsize::new(0x1000),
            )
            .unwrap()
            .cast::<LocalState>()
            .unwrap()
            .into_parts()
            .0;

        // Write the LPU structure into memory.
        lpu_ptr.write(LocalState {
            clock: AtomicClock::new(),
            int_ctrl: InterruptController::create(),
        });

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        MSR::IA32_FS_BASE.write(lpu_ptr as u64 | apic_id);
        int_ctrl().unwrap().sw_enable();
    }
}

struct LocalState {
    clock: AtomicClock,
    int_ctrl: InterruptController,
}

fn get_fs_base() -> Option<u64> {
    let fs_base = MSR::IA32_FS_BASE.read();
    if fs_base > 0 {
        Some(fs_base)
    } else {
        None
    }
}

fn try_get_lpu() -> Option<&'static LocalState> {
    get_fs_base().and_then(|fs_base| unsafe { ((fs_base & !0xFFF) as *const LocalState).as_ref() })
}

pub fn is_bsp() -> bool {
    MSR::IA32_APIC_BASE.read().get_bit(8)
}

pub fn id() -> Option<u8> {
    get_fs_base().map(|fs_base| fs_base.get_bits(4..12) as u8)
}

pub fn clock() -> Option<&'static AtomicClock> {
    try_get_lpu().map(|lpu| &lpu.clock)
}

pub fn int_ctrl() -> Option<&'static InterruptController> {
    try_get_lpu().map(|lpu| &lpu.int_ctrl)
}
