use core::ffi::c_void;

#[repr(transparent)]
pub struct EFIHandle(*mut c_void);

#[repr(C)]
pub struct EFITableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    _reserved: u32
}


#[repr(C)]
pub struct EFISystemTable {
    pub header: EFITableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: EFIHandle,
    _con_in: usize,
    pub console_out_handle: EFIHandle,
    pub console_out: *mut EFISimpleOutProtocol,
    pub standard_error_handle: EFIHandle,
    _std_err: usize
}

#[repr(usize)]
pub enum EFIStatus {
    SUCCESS = 0
}

#[repr(C)]
pub struct EFISimpleOutProtocol {
    pub reset: unsafe extern "win64" fn(this: &EFISimpleOutProtocol, extended: bool) -> EFIStatus,
    pub output_string: unsafe extern "win64" fn(this: &EFISimpleOutProtocol, string: *const u16) -> EFIStatus
}