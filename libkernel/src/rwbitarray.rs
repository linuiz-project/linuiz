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
    element_count: usize,
    phantom: PhantomData<BV>,
}

impl<'arr, BV: BitValue> RwBitArray<'arr, BV> {
    const SECTION_LEN: usize = core::mem::size_of::<usize>() * 8;

    pub const fn element_bit_length_hint(element_count: usize) -> usize {
        element_count * BV::BIT_WIDTH
    }

    pub const fn section_length_hint(element_count: usize) -> usize {
        Self::element_bit_length_hint(element_count) / Self::SECTION_LEN
    }

    pub fn from_slice(slice: &'arr mut [usize], element_count: usize) -> Self {
        slice.fill(0);

        Self {
            array: RwLock::new(slice),
            element_count,
            phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.element_count
    }

    fn get_index_and_offset(index: usize) -> (usize, usize) {
        let bit_index = index * BV::BIT_WIDTH;

        (
            // section index
            bit_index / Self::SECTION_LEN,
            // section offset
            bit_index % Self::SECTION_LEN,
        )
    }

    pub fn get(&self, index: usize) -> BV {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection ({} >= {})",
            index,
            self.len()
        );

        let (section_index, section_offset) = Self::get_index_and_offset(index);
        let section_value = self.array.read()[section_index];

        BV::from_usize((section_value >> section_offset) & BV::MASK)
    }

    pub fn set(&self, index: usize, new_type: BV) {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection ({} >= {})",
            index,
            self.len()
        );

        let (section_index, section_offset) = Self::get_index_and_offset(index);
        let sections_read = self.array.upgradeable_read();
        let section_value = sections_read[section_index];

        let section_bits_set = new_type.as_usize() << section_offset;
        let section_bits_nonset = section_value & !(BV::MASK << section_offset);
        sections_read.upgrade()[section_index] = section_bits_set | section_bits_nonset;
    }

    pub fn set_eq(&self, index: usize, new_type: BV, eq_type: BV) -> bool {
        assert!(
            index < self.len(),
            "index must be less than the size of the collection ({} >= {})",
            index,
            self.len()
        );

        {
            let (section_index, section_offset) = Self::get_index_and_offset(index);
            let sections_read = self.array.upgradeable_read();
            let section_value = sections_read[section_index];
            let type_actual = BV::from_usize((section_value >> section_offset) & BV::MASK);

            if type_actual != eq_type {
                return false;
            }

            let section_bits_set = new_type.as_usize() << section_offset;
            let section_bits_nonset = section_value & !(BV::MASK << section_offset);
            sections_read.upgrade()[section_index] = section_bits_set | section_bits_nonset;
        }

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

    pub fn iter<'outer>(&'outer self) -> RwBitArrayIterator<'outer, 'arr, BV> {
        RwBitArrayIterator {
            array: &self.array,
            section_index: 0,
            section_offset: 0,
            cur_len: 0,
            max_len: self.len(),
            phantom: PhantomData,
        }
    }
}

#[cfg(debug_assertions)]
impl<'arr, BV: BitValue + core::fmt::Debug> RwBitArray<'arr, BV> {
    pub fn debug_log_elements(&self) {
        let mut run = 0;
        let mut last_bv = BV::from_usize(0);
        for bv in self.iter() {
            if bv == last_bv {
                run += 1;
            } else {
                debug!("{:?}: {}", last_bv, run);
                last_bv = bv;
                run = 0;
            }
        }
    }
}

pub struct RwBitArrayIterator<'lock, 'arr, BV: BitValue> {
    array: &'lock RwLock<&'arr mut [usize]>,
    section_index: usize,
    section_offset: usize,
    cur_len: usize,
    max_len: usize,
    phantom: PhantomData<BV>,
}

impl<'lock, 'arr, BV: BitValue> Iterator for RwBitArrayIterator<'lock, 'arr, BV> {
    type Item = BV;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_len < self.max_len {
            let section_value = self.array.read()[self.section_index];
            let cur_offset = self.section_offset;

            self.cur_len += 1;
            self.section_offset += BV::BIT_WIDTH;
            if self.section_offset >= RwBitArray::<BV>::SECTION_LEN {
                self.section_offset = 0;
                self.section_index += 1;
            }

            Some(BV::from_usize((section_value >> cur_offset) & BV::MASK))
        } else {
            None
        }
    }
}
