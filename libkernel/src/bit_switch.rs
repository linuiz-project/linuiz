use bit_field::BitField;

pub trait BitSwitchMode {}

pub enum ReadOnly {}
impl BitSwitchMode for ReadOnly {}

pub enum ReadWrite {}
impl BitSwitchMode for ReadWrite {}

struct BitSwitch32<'val, M: BitSwitchMode> {
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
