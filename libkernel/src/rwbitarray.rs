use core::marker::PhantomData;
use spin::RwLock;

pub trait BitValue: Eq {
    const BIT_WIDTH: usize;
    const MASK: usize;

    fn as_usize(&self) -> usize;
    fn from_usize(value: usize) -> Self;
}

pub struct RwBitArray<'arr, BV>
where
    BV: BitValue,
{
    array: RwLock<&'arr mut [usize]>,
    phantom: PhantomData<BV>,
}

impl<'arr, BV: BitValue + core::fmt::Debug> RwBitArray<'arr, BV> {
    const SECTION_LEN: usize = core::mem::size_of::<usize>() * 8;

    pub const fn length_hint(element_count: usize) -> usize {
        (element_count * BV::BIT_WIDTH) / Self::SECTION_LEN
    }

    pub fn from_slice(slice: &'arr mut [usize]) -> Self {
        slice.fill(0);

        Self {
            array: RwLock::new(slice),
            phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.array.read().len() * (Self::SECTION_LEN / BV::BIT_WIDTH)
    }

    pub fn get(&self, index: usize) -> BV {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection",
        );

        let bit_index = index * BV::BIT_WIDTH;
        let section_index = bit_index / Self::SECTION_LEN;
        let section_offset = bit_index % Self::SECTION_LEN;
        let section_value = self.array.read()[section_index];

        BV::from_usize((section_value >> section_offset) & BV::MASK)
    }

    pub fn set(&self, index: usize, new_type: BV) {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection",
        );

        let bit_index = index * BV::BIT_WIDTH;
        let section_index = bit_index / Self::SECTION_LEN;
        let section_offset = bit_index % Self::SECTION_LEN;

        let sections_read = self.array.upgradeable_read();
        let section_value = sections_read[section_index];
        let section_bits_set = new_type.as_usize() << section_offset;
        let section_bits_nonset = section_value & !(BV::MASK << section_offset);

        let mut sections_write = sections_read.upgrade();
        sections_write[section_index] = section_bits_set | section_bits_nonset;
    }

    pub fn set_eq(&self, index: usize, new_type: BV, eq_type: BV) -> bool {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection",
        );

        let bit_index = index * BV::BIT_WIDTH;
        let section_index = bit_index / Self::SECTION_LEN;
        let section_offset = bit_index % Self::SECTION_LEN;

        let sections_read = self.array.upgradeable_read();
        let section_value = sections_read[section_index];
        let type_actual = BV::from_usize((section_value >> section_offset) & BV::MASK);

        if type_actual != eq_type {
            return false;
        }

        let section_bits_set = new_type.as_usize() << section_offset;
        let section_bits_nonset = section_value & !(BV::MASK << section_offset);

        let mut sections_write = sections_read.upgrade();
        sections_write[section_index] = section_bits_set | section_bits_nonset;

        debug_assert_eq!(self.get(index), new_type, "failed to set memory at index");

        true
    }

    pub fn set_eq_next(&self, new_type: BV, eq_type: BV) -> Option<usize> {
        let elements_per_section = Self::SECTION_LEN / BV::BIT_WIDTH;

        for (index, section) in self.array.write().iter_mut().enumerate() {
            for inner_index in 0..elements_per_section {
                let offset = inner_index * BV::BIT_WIDTH;
                let section_deref = *section;

                if BV::from_usize((section_deref >> offset) & BV::MASK) == eq_type {
                    let section_bits_set = new_type.as_usize() << offset;
                    let section_bits_nonset = section_deref & !(BV::MASK << offset);

                    *section = section_bits_set | section_bits_nonset;
                    return Some((index * elements_per_section) + inner_index);
                }
            }
        }

        None
    }

    // #[cfg(debug_assertions)]
    pub fn debug_log_elements(&self) {
        let mut run = 0;
        let mut last_value = BV::from_usize(0);
        for section in self.array.read().iter().map(|section| *section) {
            for offset in (0..(core::mem::size_of::<usize>() * 8)).step_by(BV::BIT_WIDTH) {
                let value = BV::from_usize((section >> offset) & BV::MASK);

                if value == last_value {
                    run += 1;
                } else {
                    debug!("{:?}: {}", last_value, run);
                    last_value = value;
                    run = 0;
                }
            }
        }
    }
}
