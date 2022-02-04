use x86_64::{instructions::segmentation::Segment, structures::tss::TaskStateSegment};

static DOUBLE_FAULT_IST: [u8; 0x1000] = [0u8; 0x1000];
pub const DOUBLE_FAULT_IST_INDEX: u16 = 6;
pub static mut TSS_STACK_PTRS: [Option<*const ()>; 7] = [
    None,
    None,
    None,
    None,
    None,
    None,
    Some(DOUBLE_FAULT_IST.as_ptr() as *const ()),
];

lazy_static::lazy_static! {
    pub static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        for (index, ptr) in unsafe {  TSS_STACK_PTRS }
                .iter()
                .enumerate()
                .filter_map(|(index, ptr)| ptr.map(|ptr| (index, ptr))) {
            tss.interrupt_stack_table[index] = x86_64::VirtAddr::from_ptr(ptr);
        }

        tss
    };
}

impl TSS {
    pub fn as_gdt_entry(&self) -> TSSEntry {
        let ptr = self as *const _ as u64;

        let base = (ptr & 0xFFFFFFFF) << 16;
        let limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;
        let ty = 0b1001 << 40;
        let low = Flags::PRESENT.bits() | base | limit | ty;
        let high = (ptr & 0xFFFFFFFF00000000) >> 32;

        TSSEntry(low, high)
    }
}

#[repr(C)]
pub struct TSSEntry(u64, u64);

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
            | Self::GANULARITY.bits()
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

#[repr(C, packed)]
pub struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

impl core::fmt::Debug for DescriptorTablePointer {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unsafe {
            formatter
                .debug_tuple("Pointer")
                .field(&((&raw const self.base).read_unaligned()))
                .field(&((&raw const self.limit).read_unaligned()))
                .finish()
        }
    }
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
        unsafe { crate::instructions::segmentation::lgdt(&self.pointer()) }
    }

    fn pointer(&self) -> DescriptorTablePointer {
        DescriptorTablePointer {
            base: self.table.as_ptr() as usize as u64,
            limit: ((self.next_free * core::mem::size_of::<u64>()) - 1) as u16,
        }
    }
}

#[derive(Debug)]
struct Selectors {
    code: u16,
    data: u16,
    tss: u16,
}

lazy_static::lazy_static! {
static ref GDT: (GlobalDescriptorTable, Selectors) = {
    let mut gdt = GlobalDescriptorTable::new();
    let code = gdt.add_entry(Entry::User(Flags::KERNEL_CODE.bits()));
    let data = gdt.add_entry(Entry::User(Flags::KERNEL_DATA.bits()));
    let tss = gdt.add_entry(Entry::tss(&TSS));

    (
        gdt,
        Selectors {
            code,
            data,
            tss,
        },
    )
};
}

#[inline]
pub fn load() {
    GDT.0.load();

    unsafe {
        crate::instructions::set_data_registers(data());
        x86_64::instructions::segmentation::CS::set_reg(core::mem::transmute(code()));
        // crate::instructions::segmentation::ltr(tss());
    }
}

pub fn pointer() -> DescriptorTablePointer {
    GDT.0.pointer()
}

pub fn code() -> u16 {
    GDT.1.code
}

pub fn data() -> u16 {
    GDT.1.data
}

pub fn tss() -> u16 {
    GDT.1.tss
}
