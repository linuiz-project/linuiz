use libkernel::bitfield_getter;

#[repr(C)]
pub struct PRDTEntry {
    db_addr_lower: u32,
    db_addr_upper: u32,
    rsvd0: u32,
    bits: u32,
}

impl PRDTEntry {
    bitfield_getter!(bits, u32, byte_count, 0..22);
    bitfield_getter!(bits, interrupt_on_completion, 31);

    pub fn set_db_addr(&mut self, addr: libkernel::Address<libkernel::Virtual>) {
        let addr_usize = addr.as_usize();

        self.db_addr_lower = addr_usize as u32;
        self.db_addr_upper = (addr_usize >> 32) as u32;
    }

    pub fn set_sector_count(&mut self, sector_count: u32) {
        self.set_byte_count(
            (sector_count << 9) - 1, /* 512-byte alignment per sector */
        );
    }

    pub fn clear(&mut self) {
        self.db_addr_lower = 0;
        self.db_addr_upper = 0;
        self.bits = 0;
    }
}

pub trait CommandFIS {}

#[repr(C)]
pub struct Command {
    bits: u16,
    prdt_len: u16,
    prdb_count: u32,
    cmd_tbl_addr_lower: u32,
    cmd_tbl_addr_upper: u32,
    rsvd0: [u8; 4],
}

impl Command {
    const COMMAND_FIS_OFFSET: usize = 0x0;
    const ATAPI_COMMAND_OFFSET: usize = 0x40;
    const PRDT_ENTRIES_OFFSET: usize = 0x80;

    bitfield_getter!(bits, u16, fis_len, 0..5);
    bitfield_getter!(bits, atapi, 5);
    bitfield_getter!(bits, write, 6);
    bitfield_getter!(bits, prefetchable, 7);
    bitfield_getter!(bits, reset, 8);
    bitfield_getter!(bits, bist, 9);
    bitfield_getter!(bits, clear_busy_on_rok, 10);
    bitfield_getter!(bits, u16, port_multiplier, 12..16);

    const fn total_command_alloc(prdt_len: u16) -> usize {
        128 /* size of command table in bytes */ + (core::mem::size_of::<PRDTEntry>() * (prdt_len as usize))
    }

    fn command_table_ptr_mut(&mut self) -> *mut u8 {
        ((self.cmd_tbl_addr_lower as usize) | ((self.cmd_tbl_addr_upper as usize) << 32)) as *mut u8
    }

    pub fn reset<F: CommandFIS + Sized>(&mut self, prdt_len: u16, fis: F) {
        let cmd_tbl_ptr = unsafe {
            alloc::alloc::alloc_zeroed(alloc::alloc::Layout::from_size_align_unchecked(
                Self::total_command_alloc(prdt_len),
                1,
            ))
        };

        self.bits = (core::mem::size_of::<F>() / core::mem::size_of::<u32>()) as u16;
        self.prdt_len = prdt_len;
        self.prdb_count = 0;
        self.cmd_tbl_addr_lower = (cmd_tbl_ptr as usize) as u32;
        self.cmd_tbl_addr_upper = ((cmd_tbl_ptr as usize) >> 32) as u32;

        *self.command_fis() = fis;
        self.prdt_entries().iter_mut().for_each(|prdt| prdt.clear());
    }

    pub fn command_fis<F: CommandFIS + Sized>(&mut self) -> &mut F {
        unsafe {
            (self.command_table_ptr_mut().add(Self::COMMAND_FIS_OFFSET) as *mut F)
                .as_mut()
                .unwrap()
        }
    }

    pub fn prdt_entries(&mut self) -> &mut [PRDTEntry] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.command_table_ptr_mut().add(Self::PRDT_ENTRIES_OFFSET) as *mut PRDTEntry,
                self.prdt_len as usize,
            )
        }
    }
}

impl Drop for Command {
    fn drop(&mut self) {
        unsafe {
            alloc::alloc::dealloc(
                self.command_table_ptr_mut(),
                alloc::alloc::Layout::from_size_align_unchecked(
                    Self::total_command_alloc(self.prdt_len),
                    1,
                ),
            );
        }
    }
}
