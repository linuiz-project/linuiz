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
    query_mode: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, mode: usize, columns: *mut usize, rows: *mut usize) -> EFIStatus,
    set_mode: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, mode: usize) -> EFIStatus,
    set_attribute: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, attribute: usize) -> EFIStatus,
    clear_screen: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol) -> EFIStatus,
    set_cursor_pos: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, column: usize, row: usize) -> EFIStatus,
    enable_cursor: unsafe extern "win64" fn(this: &EFISimpleTextOutputProtocol, visible: bool) -> EFIStatus
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

    pub fn print(&self, string: &str) -> Result<(), EFIStatus> {
        let string_length = string.len();
        
        if string_length > 512 {
                Err(EFIStatus::StringTooLong)
        } else {
            let string_bytes = string.as_bytes();
            // todo don't allocate to stack every time, use a static buffer + mutex maybe
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

    pub fn println(&self, string: &str) -> Result<(), EFIStatus> {
        match self.print(string) {
            Err(status) => Err(status),
            _ => self.print("\r\n")
        }
    }

    pub fn print_many(&self, strings: &[&str]) -> Result<(), EFIStatus> {
        let mut return_result = Ok(());
        
        for string in strings {
            let result = self.print(string);

            if result.is_err() {
                return_result = result;
                break;
            }
        }

        return_result
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

    pub fn set_attribute(&self, attribute: usize) -> Result<(), EFIStatus> {
        match unsafe { (self.set_attribute)(&self, attribute) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }

    pub fn clear(&self) -> Result<(), EFIStatus> {
        match unsafe { (self.clear_screen)(&self) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }

    pub fn set_cursor_pos(&self, column: usize, row: usize) -> Result<(), EFIStatus> {
        match unsafe { (self.set_cursor_pos)(&self, column, row) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }

    pub fn toggle_cursor_visibility(&self, visible: bool) -> Result<(), EFIStatus> {
        match unsafe { (self.enable_cursor)(&self, visible) } {
            EFIStatus::Success => Ok(()),
            status => Err(status)
        }
    }
}

impl core::fmt::Write for EFISimpleTextOutputProtocol {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        self.print(string);
        Ok(())
    }
}