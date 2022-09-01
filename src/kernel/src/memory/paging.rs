use core::fmt;
use libkernel::memory::Page;

#[cfg(target_arch = "x86_64")]
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const VALID = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        // We don't support huge pages for now.
        // const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        //  9..=11 available
        // 12..52 frame index
        // 52..=58 available
        const NO_EXECUTE = 1 << 63;

        const RO = Self::VALID.bits() | Self::NO_EXECUTE.bits();
        const RW = Self::VALID.bits() | Self::WRITABLE.bits() | Self::NO_EXECUTE.bits();
        const RX = Self::VALID.bits();
        const PTE = Self::VALID.bits() | Self::WRITABLE.bits() | Self::USER.bits();
        const MMIO = Self::RW.bits() | Self::UNCACHEABLE.bits();
    }
}

#[cfg(target_arch = "riscv64")]
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const VALID = 1 << 0;
        const READ = 1 << 1;
        const WRITE = 1 << 2;
        const EXECUTE = 1 << 3;
        const USER = 1 << 4;
        const GLOBAL = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY = 1 << 7;

        const RO = Self::VALID.bits() | Self::READ.bits();
        const RW = Self::VALID.bits() | Self::READ.bits() | Self::WRITE.bits();
        const RX = Self::VALID.bits() | Self::READ.bits() | Self::EXECUTE.bits();
        const PTE = Self::VALID.bits() | Self::READ.bits() | Self::WRITE.bits();
        const MMIO = Self::RW.bits();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeModify {
    Set,
    Insert,
    Remove,
    Toggle,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    #[cfg(target_arch = "x86_64")]
    const FRAME_INDEX_MASK: u64 = 0x000FFFFF_FFFFF000;
    #[cfg(target_arch = "riscv64")]
    const FRAME_INDEX_MASK: u64 = 0x003FFFFF_FFFFFC00;

    const FRAME_INDEX_SHIFT: u32 = Self::FRAME_INDEX_MASK.trailing_zeros();

    /// Returns an empty `Self`. All bits of this entry will be 0.
    #[inline(always)]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Whether the page table entry is present or usable the memory controller.
    #[inline(always)]
    pub const fn is_present(&self) -> bool {
        self.get_attributes().contains(PageAttributes::VALID)
    }

    /// Gets the frame index of the page table entry.
    #[inline(always)]
    pub const fn get_frame_index(&self) -> usize {
        ((self.0 & Self::FRAME_INDEX_MASK) >> Self::FRAME_INDEX_SHIFT) as usize
    }

    /// Sets the entry's frame index.
    #[inline(always)]
    pub fn set_frame_index(&mut self, frame_index: usize) {
        self.0 = (self.0 & !Self::FRAME_INDEX_MASK) | ((frame_index as u64) << Self::FRAME_INDEX_SHIFT);
    }

    /// Takes this page table entry's frame, even if it is non-present.
    #[inline]
    pub unsafe fn take_frame_index(&mut self) -> usize {
        let frame_index = self.get_frame_index();
        self.0 &= !Self::FRAME_INDEX_MASK;
        frame_index
    }

    /// Gets the attributes of this page table entry.
    #[inline(always)]
    pub const fn get_attributes(&self) -> PageAttributes {
        PageAttributes::from_bits_truncate(self.0)
    }

    /// Sets the attributes of this page table entry.
    pub fn set_attributes(&mut self, new_attributes: PageAttributes, modify_mode: AttributeModify) {
        let mut attributes = PageAttributes::from_bits_truncate(self.0);

        match modify_mode {
            AttributeModify::Set => attributes = new_attributes,
            AttributeModify::Insert => attributes.insert(new_attributes),
            AttributeModify::Remove => attributes.remove(new_attributes),
            AttributeModify::Toggle => attributes.toggle(new_attributes),
        }

        #[cfg(target_arch = "x86_64")]
        if !crate::arch::x64::registers::msr::IA32_EFER::get_nxe() {
            // This bit is reserved if NXE is not supported. For now, this means silently removing it for compatability.
            attributes.remove(PageAttributes::NO_EXECUTE);
        }

        self.0 = (self.0 & !PageAttributes::all().bits()) | attributes.bits();
    }

    /// Clears the page table entry of data, setting all bits to zero.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Page Table Entry")
            .field(&format_args!("{:#X}", self.get_frame_index()))
            .field(&self.get_attributes())
            .field(&format_args!("0x{:X}", self.0))
            .finish()
    }
}

pub trait TableLevel {}

#[derive(Debug)]
pub enum Level4 {}
#[derive(Debug)]
pub enum Level3 {}
#[derive(Debug)]
pub enum Level2 {}
#[derive(Debug)]
pub enum Level1 {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}

pub trait HeirarchicalLevel: TableLevel {
    type NextLevel: TableLevel;
}
impl HeirarchicalLevel for Level4 {
    type NextLevel = Level3;
}
impl HeirarchicalLevel for Level3 {
    type NextLevel = Level2;
}
impl HeirarchicalLevel for Level2 {
    type NextLevel = Level1;
}

#[repr(C, align(0x1000))]
#[derive(Debug)]
pub struct PageTable<L: TableLevel>([PageTableEntry; 512], core::marker::PhantomData<L>);

impl<L: TableLevel> PageTable<L> {
    pub const fn new() -> Self {
        Self([PageTableEntry::empty(); 512], core::marker::PhantomData)
    }

    pub unsafe fn clear(&mut self) {
        self.0.fill(PageTableEntry::empty());
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.0[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.0[index]
    }

    pub fn iter(&self) -> core::slice::Iter<PageTableEntry> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<PageTableEntry> {
        self.0.iter_mut()
    }
}

impl<L: HeirarchicalLevel> PageTable<L> {
    /// Gets an immutable reference to the page table that lies in the given entry index's frame.
    pub unsafe fn sub_table(&self, index: usize, phys_mapped_page: &Page) -> Option<&PageTable<L::NextLevel>> {
        let entry = self.get_entry(index);

        if entry.is_present() {
            Some(
                phys_mapped_page
                    .forward_checked(entry.get_frame_index())
                    .unwrap()
                    .address()
                    .as_ptr::<PageTable<L::NextLevel>>()
                    .as_ref()
                    .unwrap(),
            )
        } else {
            None
        }
    }

    /// Gets an mutable reference to the page table that lies in the given entry index's frame.
    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        phys_mapped_page: &Page,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        let entry = self.get_entry(index);

        if entry.is_present() {
            Some(
                phys_mapped_page
                    .forward_checked(entry.get_frame_index())
                    .unwrap()
                    .address()
                    .as_mut_ptr::<PageTable<L::NextLevel>>()
                    .as_mut()
                    .unwrap(),
            )
        } else {
            None
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry index's frame, or
    /// creates the sub page table if it doesn't exist.
    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        phys_mapping_page: &Page,
        frame_manager: &'static crate::memory::FrameManager,
    ) -> &mut PageTable<L::NextLevel> {
        // TODO use a `for` loop to support arbitrary page table depths
        let entry = self.get_entry_mut(index);

        let (frame_index, created) = if entry.is_present() {
            (entry.get_frame_index(), false)
        } else {
            let frame_index = frame_manager.lock_next().unwrap();

            entry.set_frame_index(frame_index);
            entry.set_attributes(PageAttributes::PTE, AttributeModify::Set);

            (frame_index, true)
        };

        let sub_table_page = phys_mapping_page.forward_checked(frame_index).unwrap();

        // If we created the sub-table page, clear it to a known initial state.
        if created {
            sub_table_page.clear_memory();
        }

        &mut *sub_table_page.address().as_mut_ptr::<PageTable<L::NextLevel>>()
    }
}
