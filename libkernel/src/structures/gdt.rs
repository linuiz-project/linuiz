use crate::{addr_ty::Virtual, Address};
use x86_64::{instructions::segmentation::Segment, structures::tss::TaskStateSegment};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static::lazy_static! {
    pub static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = x86_64::VirtAddr::from_ptr(&STACK);
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };

        tss
    };
}

struct Selectors {
    code_selector: u16,
    data_selector: u16,
    tss_selector: u16,
}

lazy_static::lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.add_entry(Entry::User(Flags::KERNEL_CODE.bits()));
    let data_selector = gdt.add_entry(Entry::User(Flags::KERNEL_DATA.bits()));
    let tss_selector = gdt.add_entry(Entry::tss(&TSS));

    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            tss_selector,
        },
    )
};}

bitflags::bitflags! {
    pub struct Flags: u64 {
        const ACCESSED = 1 << 40;
        const WRITABLE = 1 << 41;
        const CONFORMING = 1 << 42;
        const EXECUTABLE = 1 << 43;
        const USER_SEGMENT = 1 << 44;
        const PRESENT = 1 << 47;
        const LONG_MODE = 1 << 53;
        const SIZE_32 = 1 << 54;
        const GANULARITY = 1 << 55;
        const MAX_LIMIT = (0xF << 48) | 0xFFFF;
    }
}

impl Flags {
    const COMMON: Self = Self::from_bits_truncate(
        Self::USER_SEGMENT.bits()
            | Self::PRESENT.bits()
            | Self::WRITABLE.bits()
            | Self::MAX_LIMIT.bits(),
    );
    const KERNEL_CODE: Self = Self::from_bits_truncate(
        Self::COMMON.bits() | Self::EXECUTABLE.bits() | Self::LONG_MODE.bits(),
    );
    const KERNEL_DATA: Self = Self::from_bits_truncate(Self::COMMON.bits() | Self::SIZE_32.bits());
}

pub enum Entry {
    User(u64),
    System(u64, u64),
}

impl Entry {
    pub fn tss(tss: &'static TaskStateSegment) -> Entry {
        let ptr = tss as *const _ as u64;

        let base = (ptr & 0xFFFFFFFF) << 16;
        let limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;
        let ty = 0b1001 << 40;
        let low = Flags::PRESENT.bits() | base | limit | ty;
        let high = (ptr & 0xFFFFFFFF00000000) >> 32;

        Entry::System(low, high)
    }
}

#[repr(C)]
pub struct Pointer {
    limit: u16,
    base: Address<Virtual>,
}

#[repr(C)]
pub struct GlobalDescriptorTable {
    table: [u64; 8],
    next_free: usize,
}

impl GlobalDescriptorTable {
    pub const fn new() -> Self {
        Self {
            table: [0u64; 8],
            next_free: 1,
        }
    }

    /// TODO Use `Selector` rather than u16
    pub const fn add_entry(&mut self, entry: Entry) -> u16 {
        let index = match entry {
            Entry::User(bits) => self.push(bits),
            Entry::System(bits_low, bits_high) => {
                let index = self.push(bits_low);
                self.push(bits_high);

                index
            }
        };

        index * (core::mem::size_of::<u64>() as u16)
    }

    const fn push(&mut self, descriptor_bits: u64) -> u16 {
        if self.next_free < self.table.len() {
            let index = self.next_free;
            self.table[index] = descriptor_bits;
            self.next_free += 1;
            index as u16
        } else {
            panic!("GDT is full")
        }
    }

    #[inline]
    pub fn load(&self) {
        let pointer = self.pointer();
        info!("LOAD:");
        unsafe {
            for descriptor in &*core::slice::from_raw_parts(pointer.base.as_ptr::<u64>(), 8) {
                info!("0b{:b}", descriptor);
            }
        }
        unsafe { crate::instructions::segmentation::lgdt(&pointer) }
    }

    fn pointer(&self) -> Pointer {
        Pointer {
            limit: (self.next_free * core::mem::size_of::<u64>() - 1) as u16,
            base: Address::<Virtual>::from_ptr(self.table.as_ptr()),
        }
    }
}

pub fn gdt() -> &'static GlobalDescriptorTable {
    &GDT.0
}

pub fn code() -> u16 {
    GDT.1.code_selector
}

#[inline]
pub fn data() -> u16 {
    GDT.1.data_selector
}

pub fn tss() -> u16 {
    GDT.1.tss_selector
}

pub fn pointer() -> Pointer {
    GDT.0.pointer()
}
