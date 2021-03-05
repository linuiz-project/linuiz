use crate::structures::GUID;
use core::lazy::OnceCell;
use x86_64::PhysAddr;

pub struct SystemConfigTableCell {
    table: OnceCell<SystemConfigTable>,
}

unsafe impl Send for SystemConfigTableCell {}
unsafe impl Sync for SystemConfigTableCell {}

impl core::fmt::Debug for SystemConfigTableCell {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("SystemConfigTable")
            .field(match self.table.get().is_some() {
                true => &"Some",
                false => &"None",
            })
            .finish()
    }
}

impl SystemConfigTableCell {
    fn set(&self, table: SystemConfigTable) -> Result<(), SystemConfigTable> {
        self.table.set(table)
    }

    fn get<'a>(&'a self) -> Option<&'a SystemConfigTable> {
        self.table.get()
    }
}

static SYSTEM_CONFIG_TABLE: SystemConfigTableCell = SystemConfigTableCell {
    table: OnceCell::new(),
};

pub unsafe fn init_system_config_table(ptr: *const SystemConfigTableEntry, len: usize) {
    SYSTEM_CONFIG_TABLE
        .set(SystemConfigTable::new(ptr, len))
        .expect("global ACPI config table has already been set");
}

pub fn system_config_table() -> &'static SystemConfigTable {
    SYSTEM_CONFIG_TABLE
        .get()
        .expect("global ACPI configration table has not been set")
}

#[derive(Debug)]
pub struct SystemConfigTable {
    ptr: *const SystemConfigTableEntry,
    len: usize,
}

impl SystemConfigTable {
    unsafe fn new(ptr: *const SystemConfigTableEntry, len: usize) -> Self {
        Self { ptr, len }
    }

    pub fn iter(&self) -> core::slice::Iter<SystemConfigTableEntry> {
        unsafe { &*core::ptr::slice_from_raw_parts(self.ptr, self.len) }.iter()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct SystemConfigTableEntry {
    guid: GUID,
    addr: PhysAddr,
}

impl SystemConfigTableEntry {
    pub fn guid(&self) -> GUID {
        self.guid.clone()
    }

    pub fn addr(&self) -> PhysAddr {
        self.addr
    }

    pub unsafe fn as_ref<T>(&self) -> &T {
        &*(self.addr().as_u64() as *mut T)
    }
    pub unsafe fn as_mut_ref<T>(&self) -> &mut T {
        &mut *(self.addr().as_u64() as *mut T)
    }
}
