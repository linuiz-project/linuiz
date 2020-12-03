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

#[repr(usize)]
pub enum EFIStatus {
    Success = 0,
    DeviceError = 1,
    Unsupported = 2
}

#[repr(C)]
pub struct EFISimpleTextOutputMode {
    max_mode: i32,
    mode: i32,
    attribute: i32,
    cursor_column: i32,
    cursor_row: i32,
    cursor_visible: bool
}

#[repr(C)]
pub struct EFISystemTable {
    pub header: EFITableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: EFIHandle,
    _con_in: usize,
    pub console_out_handle: EFIHandle,
    console_out: *mut EFISimpleTextOutputProtocol,
    pub standard_error_handle: EFIHandle,
    _std_err: usize
}

impl EFISystemTable {
    pub fn get_console_out(&self) -> &mut EFISimpleTextOutputProtocol {
        unsafe {
            &mut *(self.console_out)
        }
    }
}

#[repr(C)]
pub struct EFISimpleTextOutputProtocol {
    reset: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, extended: bool) -> EFIStatus,
    output_string: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, string: *const u16) -> EFIStatus,
    query_mode: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, mode_number: u32, columns: *mut u32, rows: *mut u32) -> EFIStatus
}

impl EFISimpleTextOutputProtocol {
    pub fn reset(&self, extended: bool) -> EFIStatus {
        unsafe {
            (self.reset)(&self, extended)
        }
    }

    pub fn write_string(&self, string: &str) -> Option<EFIStatus> {
        let string_length = string.len();
        
        if (string_length > 512) {
            None
        } else {
        let string_bytes = string.as_bytes();
        let mut buffer = [0u16; 512];
    
        for i in 0..string_length {
            buffer[i] = string_bytes[i] as u16;
        }

        unsafe {
        (self.output_string)(&self, buffer.as_ptr());
        }
        Some(EFIStatus::Success)
    }
    }

    pub fn query_mode(&self, mode: u32) -> Result<(u32, u32), EFIStatus> {
        let columns: u32;
        let rows: u32;
        let status: EFIStatus;

        unsafe {
            let columns_ptr = 0x0 as *mut u32;
            let rows_ptr = 0x0 as *mut u32;
            status = (self.query_mode)(&self, mode, columns_ptr, rows_ptr);
            columns = *columns_ptr;
            rows = *rows_ptr;
        }

        match status {
            EFIStatus::Success => Ok((columns, rows)),
            _ => Err(status)
        }
    } 
}