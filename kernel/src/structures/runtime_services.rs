#[repr(C)]
#[derive(Debug)]
pub struct UEFIHeader {
    pub signature: u64,
    pub revision: u32,
    pub size: u32,
    pub crc: u32,
    _reserved: u32,
}

#[repr(C)]
pub struct RuntimeTable {
    header: UEFIHeader,
    fw_vendor: *const u16,
    fw_revision: u32,
    _ignored1: [usize; 6],
    runtime: &'static RuntimeServices,
    _ignored2: [usize; 3],
}

impl RuntimeTable {
    pub unsafe fn runtime_services(&self) -> &RuntimeServices {
        self.runtime
    }
}

#[repr(C)]
pub struct RuntimeServices {
    header: UEFIHeader,
    _ignored1: [usize; 10],
    reset: unsafe extern "efiapi" fn(reset_type: ResetType, status: usize, data: *const u8) -> !,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetType {
    Cold = 0,
    Warm,
    Shutdown,
    PlatformSpecific,
}
