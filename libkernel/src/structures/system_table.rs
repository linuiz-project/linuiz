use crate::{addr_ty::Physical, cell::SyncOnceCell, structures::GUID, Address};

static SYSTEM_CONFIG_TABLE: SyncOnceCell<SystemConfigTable> = SyncOnceCell::new();

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
    addr: Address<Physical>,
}

impl SystemConfigTableEntry {
    pub fn guid(&self) -> GUID {
        self.guid.clone()
    }

    pub fn addr(&self) -> Address<Physical> {
        self.addr
    }

    pub unsafe fn as_ref<T>(&self) -> &T {
        &*(self.addr().as_usize() as *mut T)
    }
    pub unsafe fn as_mut_ref<T>(&self) -> &mut T {
        &mut *(self.addr().as_usize() as *mut T)
    }
}
