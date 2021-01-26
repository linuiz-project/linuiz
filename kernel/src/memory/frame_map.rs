use spin::RwLock;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Unallocated = 0,
    Allocated,
    Reserved,
    Corrupted,
}

impl FrameType {
    pub const BIT_WIDTH: usize = 0x2;
    pub const MASK: usize = (Self::BIT_WIDTH << 1) - 1;

    pub const fn from_usize(value: usize) -> Self {
        match value {
            0 => FrameType::Unallocated,
            1 => FrameType::Allocated,
            2 => FrameType::Reserved,
            3 => FrameType::Corrupted,
            _ => panic!("invalid cast"),
        }
    }

    pub const fn as_usize(&self) -> usize {
        match self {
            FrameType::Unallocated => 0,
            FrameType::Allocated => 1,
            FrameType::Reserved => 2,
            FrameType::Corrupted => 3,
        }
    }
}

pub struct FrameMap<'arr> {
    array: RwLock<&'arr mut [usize]>,
    len: usize,
}

impl FrameMap<'_> {
    const SECTION_SIZE: usize = core::mem::size_of::<usize>() * 8;

    pub const fn size_hint_bits(len: usize) -> usize {
        len * FrameType::BIT_WIDTH
    }

    pub const fn size_hint_bytes(len: usize) -> usize {
        Self::size_hint_bits(len) / 8
    }

    pub const fn size_hint_sections(len: usize) -> usize {
        Self::size_hint_bits(len / Self::SECTION_SIZE)
    }

    pub unsafe fn from_ptr(ptr: *mut usize, len: usize) -> Self {
        let array = &mut *core::ptr::slice_from_raw_parts_mut(ptr, Self::size_hint_sections(len));
        core::ptr::write_bytes(ptr, 0x0, array.len());

        Self {
            array: RwLock::new(array),
            len,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn get(&self, index: usize) -> FrameType {
        if index < self.len() {
            let element_index = index * FrameType::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);
            let section_value = self.array.read()[section_index];

            FrameType::from_usize((section_value >> section_offset) & FrameType::MASK)
        } else {
            panic!(
                "index must be less than the size of the collection !({} < {})",
                index,
                self.len()
            );
        }
    }

    pub fn set(&self, index: usize, mem_type: FrameType) {
        if index < self.len() {
            let element_index = index * FrameType::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);

            let sections_read = self.array.upgradeable_read();
            let section_value = sections_read[section_index];
            let section_bits_set = mem_type.as_usize() << section_offset;
            let section_bits_nonset = section_value & !(FrameType::MASK << section_offset);

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

    pub fn set_eq(&self, index: usize, mem_type: FrameType, eq_type: FrameType) -> bool {
        if index < self.len() {
            let element_index = index * FrameType::BIT_WIDTH;
            let section_index = element_index / Self::SECTION_SIZE;
            let section_offset = element_index - (section_index * Self::SECTION_SIZE);

            let sections_read = self.array.upgradeable_read();
            let section_value = sections_read[section_index];
            let mem_type_actual =
                FrameType::from_usize((section_value >> section_offset) & FrameType::MASK);

            if mem_type_actual != eq_type {
                return false;
            }

            let section_bits_set = mem_type.as_usize() << section_offset;
            let section_bits_nonset = section_value & !(FrameType::MASK << section_offset);

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
