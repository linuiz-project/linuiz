use crate::{ReadOnly, ReadWrite};
use bit_field::BitField;

pub trait BitSwitchMode {}
impl BitSwitchMode for ReadOnly {}
impl BitSwitchMode for ReadWrite {}

pub struct BitSwitch32<'val, M: BitSwitchMode> {
    val: &'val mut u32,
    bit_index: u8,
    phantom: core::marker::PhantomData<M>,
}

impl<M: BitSwitchMode> BitSwitch32<'_, M> {
    pub fn get(&self) -> bool {
        self.val.get_bit(self.bit_index as usize)
    }
}

impl<'val> BitSwitch32<'val, ReadOnly> {
    pub fn new(val: &'val u32, bit_index: u8) -> Self {
        Self {
            val: unsafe { &mut *(val as *const u32 as *mut u32) },
            bit_index,
            phantom: core::marker::PhantomData,
        }
    }
}

impl core::fmt::Debug for BitSwitch32<'_, ReadOnly> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("BitSwitch<ReadOnly>")
            .field(&self.get())
            .finish()
    }
}

impl<'val> BitSwitch32<'val, ReadWrite> {
    pub fn new(val: &'val mut u32, bit_index: u8) -> Self {
        Self {
            val: unsafe { &mut *(val as *const u32 as *mut u32) },
            bit_index,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn set(&mut self, set: bool) {
        self.val.set_bit(self.bit_index as usize, set);
    }
}

impl core::fmt::Debug for BitSwitch32<'_, ReadWrite> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("BitSwitch<ReadWrite>")
            .field(&self.get())
            .finish()
    }
}
