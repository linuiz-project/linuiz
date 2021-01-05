const SECTION_BITS_COUNT: usize = core::mem::size_of::<usize>() * 8;

struct Section {
    pub(self) index: usize,
    pub(self) bit_offset: usize,
    pub(self) value: usize,
}

pub struct BitArray<'arr> {
    array: &'arr mut [usize],
}

impl<'arr> BitArray<'arr> {
    pub fn from_ptr(ptr: *mut usize, length: usize) -> Self {
        let array =
            unsafe { &mut *core::ptr::slice_from_raw_parts_mut(ptr, length / SECTION_BITS_COUNT) };
        // clear the array
        for index in 0..array.len() {
            array[index] = 0;
        }

        Self { array }
    }
    pub fn get_bit(&self, index: usize) -> Option<bool> {
        if index < self.length() {
            let section = self.get_section(index);
            let section_bit = section.value & (1 << section.bit_offset);

            Some(section_bit != 0)
        } else {
            None
        }
    }

    pub fn set_bit(&mut self, index: usize, set: bool) -> Option<bool> {
        if index < self.length() {
            let section = self.get_section(index);
            let section_bit_mask = section.value & !(1 << section.bit_offset);
            let section_bit_set = (set as usize) << section.bit_offset;

            self.array[section.index] = section_bit_mask | section_bit_set;
            Some(set)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_section(&self, index: usize) -> Section {
        let section_index = index / SECTION_BITS_COUNT;
        let section_offset = index - (section_index * SECTION_BITS_COUNT);
        let section = self.array[section_index];

        Section {
            index: section_index,
            bit_offset: section_offset,
            value: section,
        }
    }

    pub fn length(&self) -> usize {
        self.array.len() * SECTION_BITS_COUNT
    }
}
