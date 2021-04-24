use crate::{
    addr_ty::Physical,
    cell::SyncOnceCell,
    memory::mmio::{Mapped, MMIO},
    registers::MSR,
    Address,
};
use core::{intrinsics::transmute, marker::PhantomData};

static mut LOCAL_APIC: SyncOnceCell<APIC> = SyncOnceCell::new();

pub fn load() {
    if unsafe { LOCAL_APIC.get().is_some() } {
        panic!("Local APIC has already been configured");
    } else {
        debug!("Loading local APIC table.");
        let start_index = APIC::mmio_addr().frame_index();
        debug!("APIC MMIO mapping at frame: {}", start_index);

        let mmio = crate::memory::mmio::unmapped_mmio(unsafe {
            crate::memory::falloc::get()
                .acquire_frames(start_index, 1, crate::memory::falloc::FrameState::MMIO)
                .unwrap()
        })
        .unwrap()
        .automap();

        unsafe { LOCAL_APIC.set(APIC::new(mmio)).ok() };
    }
}

pub fn local_apic() -> Option<&'static APIC> {
    unsafe { LOCAL_APIC.get() }
}

pub fn local_apic_mut() -> Option<&'static mut APIC> {
    unsafe { LOCAL_APIC.get_mut() }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICRegister {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    LDR = 0xD0,
    DFR = 0xE0,
    ICRL = 0x300,
    ICRH = 0x310,
    TimerInitialCount = 0x380,
    TimerCurrentCount = 0x390,
    TimerDivisor = 0x3E0,
    Last = 0x38F,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerMode {
    OneShot = 0b00,
    Periodic = 0b01,
    TSC_Deadline = 0b10,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerDivisor {
    Div2 = 0b0000,
    Div4 = 0b0001,
    Div8 = 0b0010,
    Div16 = 0b0011,
    Div32 = 0b1000,
    Div64 = 0b1001,
    Div128 = 0b1010,
    Div1 = 0b1011,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICDeliveryMode {
    Fixed = 0b000,
    SystemManagement = 0b010,
    NonMaskable = 0b100,
    External = 0b111,
    INIT = 0b101,
}

bitflags::bitflags! {
    pub struct APICErrorStatusFlags: u8 {
        const SEND_CHECKSUM_ERROR = 1 << 0;
        const RECEIVE_CHECKSUM_ERROR = 1 << 1;
        const SEND_ACCEPT_ERROR = 1 << 2;
        const RECEIVE_ACCEPT_ERROR = 1 << 3;
        const REDIRECTABLE_IPI = 1 << 4;
        const SENT_ILLEGAL_VECTOR = 1 << 5;
        const RECEIVED_ILLEGAL_VECTOR = 1 << 6;
        const ILLEGAL_REGISTER_ADDRESS = 1 << 7;
    }
}

pub struct APIC {
    mmio: MMIO<Mapped>,
}

impl APIC {
    // The `mask` bit for an LVT entry.
    pub const DISABLE: u32 = 0x10000;
    pub const SW_ENABLE: u32 = 0x100;
    pub const CPU_FOCUS: u32 = 0x200;

    pub fn mmio_addr() -> Address<Physical> {
        Address::<Physical>::new((MSR::IA32_APIC_BASE.read().get_bits(12..35) << 12) as usize)
    }

    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        Self { mmio }
    }

    pub fn is_enabled(&self) -> bool {
        (MSR::IA32_APIC_BASE.read() & (1 << 11)) > 0
    }

    pub unsafe fn enable(&mut self) {
        MSR::IA32_APIC_BASE.write_bit(11, true);
    }

    pub unsafe fn disable(&mut self) {
        MSR::IA32_APIC_BASE.write_bit(11, false);
    }

    pub fn end_of_interrupt(&mut self) {
        unsafe { self.mmio.write(0xB0, 0).unwrap() };
    }

    pub fn cmci(&mut self) -> &mut LVTRegister<Generic> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x2F0).unwrap()) }
    }

    pub fn timer(&mut self) -> &mut LVTRegister<Timer> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x320).unwrap()) }
    }

    pub fn thermal_sensor(&mut self) -> &mut LVTRegister<Generic> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x330).unwrap()) }
    }

    pub fn performance(&mut self) -> &mut LVTRegister<Generic> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x340).unwrap()) }
    }

    pub fn lint0(&mut self) -> &mut LVTRegister<LINT> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x350).unwrap()) }
    }

    pub fn lint1(&mut self) -> &mut LVTRegister<LINT> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x360).unwrap()) }
    }

    pub fn error(&mut self) -> &mut LVTRegister<Error> {
        unsafe { transmute(self.mmio.read_mut::<u32>(0x370).unwrap()) }
    }

    pub fn write_spurious(&mut self, vector: u8, enabled: bool) {
        unsafe {
            self.mmio
                .write(0xF0, (vector as u32) | ((enabled as u32) << 8))
                .unwrap()
        };
    }

    pub fn error_status(&self) -> APICErrorStatusFlags {
        APICErrorStatusFlags::from_bits_truncate(unsafe { *self.mmio.read(0x280).unwrap() })
    }

    pub unsafe fn reset(&mut self) {
        self[APICRegister::DFR] = 0xFFFFFFFF;
        let mut ldr = self[APICRegister::LDR];
        ldr &= 0xFFFFFF;
        ldr = (ldr & !0xFF) | ((ldr & 0xFF) | 1);
        self[APICRegister::LDR] = ldr;
        self.timer().set_masked(true);
        self.performance()
            .set_delivery_mode(APICDeliveryMode::NonMaskable);
        // self.lint0().set_masked(true);
        self.lint1().set_masked(true);
        self[APICRegister::TaskPriority] = 0;
        self[APICRegister::TimerInitialCount] = 0;
    }
}

impl core::ops::Index<APICRegister> for APIC {
    type Output = u32;

    fn index(&self, register: APICRegister) -> &Self::Output {
        unsafe { self.mmio.read(register as usize).unwrap() }
    }
}

impl core::ops::IndexMut<APICRegister> for APIC {
    fn index_mut(&mut self, register: APICRegister) -> &mut Self::Output {
        unsafe { self.mmio.read_mut(register as usize).unwrap() }
    }
}

pub trait LVTRegisterVariant {}

pub enum Timer {}
impl LVTRegisterVariant for Timer {}

pub enum Generic {}
impl LVTRegisterVariant for Generic {}

pub enum LINT {}
impl LVTRegisterVariant for LINT {}

pub enum Error {}
impl LVTRegisterVariant for Error {}

use bit_field::BitField;

#[repr(transparent)]
pub struct LVTRegister<T: LVTRegisterVariant> {
    value: u32,
    phantom_generic: PhantomData<T>,
}

impl<T: LVTRegisterVariant> LVTRegister<T> {
    const INTERRUPTED_OFFSET: u32 = 12;
    const INTERRUPTED_BIT: u32 = 1 << Self::INTERRUPTED_OFFSET;
    const MASKED_OFFSET: u32 = 16;
    const MASKED_BIT: u32 = 1 << Self::MASKED_OFFSET;
    const VECTOR_MASK: u32 = 0xFF;

    fn value_ptr(&self) -> *const u32 {
        (&self.value) as *const u32
    }

    fn value_mut_ptr(&mut self) -> *mut u32 {
        (&mut self.value) as *mut u32
    }

    pub fn is_interrupted(&self) -> bool {
        unsafe { (self.value_ptr().read_volatile() & Self::INTERRUPTED_BIT) > 0 }
    }

    pub fn is_masked(&self) -> bool {
        unsafe { (self.value_ptr().read_volatile() & Self::MASKED_BIT) > 0 }
    }

    pub fn set_masked(&mut self, masked: bool) {
        unsafe {
            self.value_mut_ptr().write_volatile(
                (self.value_ptr().read_volatile() & !Self::MASKED_BIT)
                    | ((masked as u32) << Self::MASKED_OFFSET),
            );
        }
    }

    pub fn get_vector(&self) -> u8 {
        unsafe { (self.value_ptr().read_volatile() & Self::VECTOR_MASK) as u8 }
    }

    pub fn set_vector(&mut self, vector: u8) {
        unsafe {
            self.value_mut_ptr().write_volatile(
                (self.value_ptr().read_volatile() & !Self::VECTOR_MASK) | vector as u32,
            );
        }
    }

    #[cfg(debug_assertions)]
    pub fn read_raw(&self) -> u32 {
        unsafe { self.value_ptr().read_volatile() }
    }
}

impl LVTRegister<Timer> {
    pub fn set_mode(&mut self, mode: APICTimerMode) {
        unsafe {
            self.value_mut_ptr().write_volatile(
                (self.value_ptr().read_volatile() & !(0b11 << 17)) | ((mode as u32) << 17),
            );
        }
    }
}

impl LVTRegister<Generic> {
    #[inline]
    pub fn set_delivery_mode(&mut self, mode: APICDeliveryMode) {
        unsafe {
            self.value_mut_ptr().write_volatile(
                (self.value_ptr().read_volatile() & !(0x111 << 8)) | ((mode as u32) << 8),
            );
        }
    }
}
