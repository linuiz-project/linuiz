pub mod rdsp;

use crate::{addr_ty::Physical, cell::SyncOnceCell, structures::GUID, Address};

pub const ACPI_GUID: GUID = GUID::new(
    0xeb9d2d30,
    0x2d88,
    0x11d3,
    0x9a16,
    [0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);

pub const ACPI2_GUID: GUID = GUID::new(
    0x8868e871,
    0xe4f1,
    0x11d3,
    0xbc22,
    [0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
);

pub(crate) trait ACPITable {
    fn body_len(&self) -> usize;
}

pub(crate) trait SizedACPITable<H, E>: ACPITable {
    fn entries(&self) -> &[E] {
        unsafe {
            &*core::ptr::slice_from_raw_parts(
                (self as *const _ as *const u8).add(core::mem::size_of::<H>()) as *const E,
                self.body_len() / core::mem::size_of::<E>(),
            )
        }
    }
}

pub(crate) trait UnsizedACPITable<H, E> {
    fn first_entry_ptr(&self) -> *const E {
        unsafe { (self as *const _ as *const u8).add(core::mem::size_of::<H>()) as *const E }
    }
}

pub trait Checksum: Sized {
    fn bytes_len(&self) -> usize {
        core::mem::size_of::<Self>()
    }

    fn checksum(&self) -> bool {
        let mut sum: u8 = 0;

        unsafe {
            &*core::ptr::slice_from_raw_parts(
                core::mem::transmute::<&Self, *const u8>(self),
                self.bytes_len(),
            )
        }
        .iter()
        .for_each(|byte| sum = sum.wrapping_add(*byte));

        sum == 0
    }

    fn checksum_panic(&self) {
        assert!(self.checksum(), "checksum invalid");
    }
}

#[repr(C)]
pub struct SDTHeader {
    signature: [u8; 4],
    len: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

impl SDTHeader {
    pub fn signature(&self) -> &str {
        core::str::from_utf8(&self.signature).expect("invalid ascii sequence for signature")
    }

    pub const fn table_len(&self) -> u32 {
        self.len
    }

    pub const fn revision(&self) -> u8 {
        self.revision
    }

    pub fn oem_id(&self) -> &str {
        core::str::from_utf8(&self.oem_id).expect("invalid ascii sequence for OEM ID")
    }

    pub fn oem_table_id(&self) -> &str {
        core::str::from_utf8(&self.oem_table_id).expect("invalid ascii sequence for OEM table ID")
    }
}

impl Checksum for SDTHeader {}

impl core::fmt::Debug for SDTHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("System Description Table Header")
            .field("Signature", &self.signature())
            .field("OEM ID", &self.oem_id())
            .field("Revision", &self.revision())
            .finish()
    }
}

static SYSTEM_CONFIG_TABLE: SyncOnceCell<SystemConfigTable> = SyncOnceCell::new();

pub unsafe fn init_system_config_table(ptr: *const SystemConfigTableEntry, len: usize) {
    SYSTEM_CONFIG_TABLE
        .set(SystemConfigTable::from_ptr(ptr, len))
        .expect("global ACPI config table has already been set");
}

pub fn get_system_config_table_entry(guid: GUID) -> Option<&'static SystemConfigTableEntry> {
    SYSTEM_CONFIG_TABLE
        .get()
        .expect("global ACPI configration table has not been set")
        .iter()
        .find(|entry| entry.guid() == guid)
}

#[derive(Debug)]
pub struct SystemConfigTable {
    ptr: *const SystemConfigTableEntry,
    len: usize,
}

impl SystemConfigTable {
    unsafe fn from_ptr(ptr: *const SystemConfigTableEntry, len: usize) -> Self {
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
