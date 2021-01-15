use core::ops::Range;

pub const SECTION_BITS_COUNT: usize = core::mem::size_of::<usize>() * 8;

struct Section {
    pub(self) index: usize,
    pub(self) bit_offset: usize,
    pub(self) value: usize,
}

pub struct BitArray<'arr> {
    array: &'arr mut [usize],
}

impl BitArray<'_> {
    pub const fn empty() -> Self {
        Self {
            array: &mut [0usize; 0],
        }
    }

    pub fn from_ptr(ptr: *mut usize, bits: usize) -> Self {
        let array = unsafe {
            &mut *core::ptr::slice_from_raw_parts_mut(ptr, (bits / SECTION_BITS_COUNT) + 1)
        };
        // clear the array
        for index in 0..array.len() {
            array[index] = 0;
        }

        Self { array }
    }

    pub fn set_bit(&mut self, index: usize, set: bool) -> Option<bool> {
        if index < self.bit_count() {
            let section = self.get_section(index);
            let section_bits_nonset = section.value & !(1 << section.bit_offset);
            let section_bit_set = (set as usize) << section.bit_offset;

            self.array[section.index] = section_bits_nonset | section_bit_set;
            assert!(self.get_bit(index).unwrap() == set);
            Some(set)
        } else {
            None
        }
    }

    pub fn get_bit(&self, index: usize) -> Option<bool> {
        if index < self.bit_count() {
            let section = self.get_section(index);
            let section_bit = section.value & (1 << section.bit_offset);

            Some(section_bit != 0)
        } else {
            None
        }
    }

    pub fn get_bits(&self, range: Range<usize>) -> BitArrayIterator {
        if range.end > self.bit_count() {
            panic!("range exceeds collection bounds");
        }

        BitArrayIterator {
            bitarray: self,
            index: range.start,
            end: range.end,
        }
    }

    #[inline(always)]
    fn get_section(&self, index: usize) -> Section {
        let section_index = index / SECTION_BITS_COUNT;
        let section_offset = index - (section_index * SECTION_BITS_COUNT);
        let section_value = self.array[section_index];

        Section {
            index: section_index,
            bit_offset: section_offset,
            value: section_value,
        }
    }

    pub fn bit_count(&self) -> usize {
        self.array.len() * SECTION_BITS_COUNT
    }

    pub fn byte_count(&self) -> usize {
        self.array.len() * core::mem::size_of::<usize>()
    }

    pub fn iter(&self) -> BitArrayIterator {
        BitArrayIterator {
            bitarray: self,
            index: 0,
            end: self.bit_count(),
        }
    }
}

pub struct BitArrayIterator<'arr> {
    bitarray: &'arr BitArray<'arr>,
    index: usize,
    end: usize,
}

impl Iterator for BitArrayIterator<'_> {
    type Item = bool;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.bitarray.bit_count(), Some(self.bitarray.bit_count()))
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.end {
            if let Some(item) = self.bitarray.get_bit(self.index) {
                self.index += 1;
                return Some(item);
            }
        }

        None
    }
}

impl ExactSizeIterator for BitArrayIterator<'_> {}
