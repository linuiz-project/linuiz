use core::{marker::PhantomData, ops::Range};
use spin::RwLock;

pub trait BitValue {
    const BIT_WIDTH: usize;
    const MASK: usize;

    fn as_usize(&self) -> usize;
    fn from_usize(value: usize) -> Self;
}

pub struct BitArray<'arr, BV>
where
    BV: BitValue + Eq,
{
    array: RwLock<&'arr mut [usize]>,
    phantom: PhantomData<BV>,
}

impl<'arr, BV: BitValue + Eq> BitArray<'arr, BV> {
    const SECTION_SIZE: usize = core::mem::size_of::<usize>() * 8;

    pub const fn length_hint(element_count: usize) -> usize {
        (element_count * BV::BIT_WIDTH) / Self::SECTION_SIZE
    }

    pub fn from_slice(slice: &'arr mut [usize]) -> Self {
        slice.iter_mut().for_each(|section| *section = 0);

        Self {
            array: RwLock::new(slice),
            phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.array.read().len() * (Self::SECTION_SIZE * BV::BIT_WIDTH)
    }

    pub fn get(&self, index: usize) -> BV {
        if index < self.len() {
            let element_index = index * BV::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);
            let section_value = self.array.read()[section_index];

            BV::from_usize((section_value >> section_offset) & BV::MASK)
        } else {
            panic!(
                "index must be less than the size of the collection !({} < {})",
                index,
                self.len()
            );
        }
    }

    pub fn set(&self, index: usize, mem_type: BV) {
        if index < self.len() {
            let element_index = index * BV::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);

            let sections_read = self.array.upgradeable_read();
            let section_value = sections_read[section_index];
            let section_bits_set = mem_type.as_usize() << section_offset;
            let section_bits_nonset = section_value & !(BV::MASK << section_offset);

            let mut sections_write = sections_read.upgrade();
            sections_write[section_index] = section_bits_set | section_bits_nonset;
        } else {
            panic!(
                "index must be less than the size of the collection !({} < {})",
                index,
                self.len()
            );
        }
    }

    pub fn set_eq(&self, index: usize, mem_type: BV, eq_type: BV) -> bool {
        if index < self.len() {
            let element_index = index * BV::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);

            let sections_read = self.array.upgradeable_read();
            let section_value = sections_read[section_index];
            let mem_type_actual = BV::from_usize((section_value >> section_offset) & BV::MASK);

            if mem_type_actual != eq_type {
                return false;
            }

            let section_bits_set = mem_type.as_usize() << section_offset;
            let section_bits_nonset = section_value & !(BV::MASK << section_offset);

            let mut sections_write = sections_read.upgrade();
            sections_write[section_index] = section_bits_set | section_bits_nonset;
        } else {
            panic!(
                "index must be less than the size of the collection !({} < {})",
                index,
                self.len()
            );
        }

        true
    }
}
