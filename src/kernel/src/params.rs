use core::ffi::CStr;
use limine::{request::ExecutableCmdlineRequest, response::ExecutableCmdlineResponse};
use spin::Once;

static PARAMS: Once<Parameters> = Once::new();

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    /// Whether the kernel should utilize multi-processing.
    pub use_multiprocessing: bool,

    /// Whether to keep the kernel symbol info (for stack traces).
    pub keep_symbol_info: bool,

    /// Whether the kernel should use low-memory mode.
    pub low_memory_mode: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            use_multiprocessing: true,
            keep_symbol_info: true,
            low_memory_mode: false,
        }
    }
}

pub fn parse(kernel_cmdline_request: &ExecutableCmdlineRequest) {
    fn parse_impl(kernel_cmdline_request: &ExecutableCmdlineRequest) -> Parameters {
        let mut params = Parameters::default();

        match kernel_cmdline_request
            .get_response()
            .map(ExecutableCmdlineResponse::cmdline)
            .map(CStr::to_str)
        {
            Some(Ok("")) => {
                // Ignore accidental extra spaces
            }

            Some(Ok("--nomp")) => params.use_multiprocessing = false,

            Some(Ok("--keep-symbols")) => params.keep_symbol_info = true,

            Some(Ok("--lomem")) => params.low_memory_mode = true,

            Some(Ok(arg)) => {
                warn!("Unknown command line argument: {arg:?}");
            }

            Some(Err(error)) => {
                error!("Failed to parse kernel command line: {error:?}");
            }

            None => {
                warn!("Bootloader didn't provide response to kernel command line request.");
            }
        }

        debug!("Kernel Parameters:\n{params:#?}");

        params
    }

    PARAMS.call_once(|| parse_impl(kernel_cmdline_request));
}

pub fn use_multiprocessing() -> bool {
    PARAMS.wait().use_multiprocessing
}

pub fn keep_symbol_info() -> bool {
    PARAMS.wait().keep_symbol_info
}

pub fn use_low_memory() -> bool {
    PARAMS.wait().low_memory_mode
}
