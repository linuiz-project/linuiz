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
    DeviceError,
    Unsupported,
    StringTooLong
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
    query_mode: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, mode_number: usize, columns: *mut usize, rows: *mut usize) -> EFIStatus,
    set_mode: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, mode_number: usize) -> EFIStatus,
}

impl EFISimpleTextOutputProtocol {
    pub fn reset(&self, extended: bool) -> Result<(), EFIStatus> {
        unsafe {
            match (self.reset)(&self, extended) {
                EFIStatus::Success => Ok(()),
                status => Err(status)
            }
        }
    }

    pub fn write_string(&self, string: &str) -> Result<(), EFIStatus> {
        let string_length = string.len();
        
        if string_length > 512 {
                Err(EFIStatus::StringTooLong)
        } else {
        let string_bytes = string.as_bytes();
        let mut buffer = [0u16; 512];
    
        for i in 0..string_length {
            buffer[i] = string_bytes[i] as u16;
        }


        match unsafe { (self.output_string)(&self, buffer.as_ptr()) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }
    }

    pub fn query_mode(&self, mode: usize) -> Result<(usize, usize), EFIStatus> {
        let columns = 0 as *mut usize;
        let rows = 0 as *mut usize;

        unsafe {
        match (self.query_mode)(&self, mode, columns, rows) {
            EFIStatus::Success => Ok((*columns, *rows)),
            status => Err(status)
        }
    }
    }

    pub fn set_mode(&self, mode: usize) -> Result<(), EFIStatus> {
        match unsafe { (self.set_mode)(&self, mode) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }
}