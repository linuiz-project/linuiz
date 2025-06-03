static PARAMS: spin::Once<Parameters> = spin::Once::new();

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    /// Whether the kernel should utilize multi-processing.
    pub use_multiprocessing: bool,

    /// Whether to keep the kernel symbol info before reclaiming extra memory.
    pub drop_symbol_info: bool,

    /// Whether the kernel should use low-memory mode.
    pub low_memory_mode: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            use_multiprocessing: true,
            drop_symbol_info: false,
            low_memory_mode: false,
        }
    }
}

pub fn parse(kernel_cmdline_request: &limine::request::ExecutableCmdlineRequest) {
    PARAMS.call_once(|| {
        let mut params = Parameters::default();

        match kernel_cmdline_request
            .get_response()
            .map(limine::response::ExecutableCmdlineResponse::cmdline)
            .map(core::ffi::CStr::to_str)
        {
            Some(Ok("")) => {
                // Ignore accidental extra spaces
            }

            Some(Ok("--nomp")) => params.use_multiprocessing = false,

            Some(Ok("--lomem")) => params.low_memory_mode = true,

            Some(Ok(arg)) => {
                warn!("Unknown command line argument: {arg:?}");
            }

            Some(Err(error)) => {
                error!("Failed to parse kernel command line: {error:?}");
            }

            None => {
                error!("Bootloader didn't provide response to kernel command line request.");
            }
        }

        debug!("Kernel Parameters:\n{params:#?}");

        params
    });
}

pub fn use_multiprocessing() -> bool {
    PARAMS.get().unwrap().use_multiprocessing
}

pub fn use_low_memory() -> bool {
    PARAMS.get().unwrap().low_memory_mode
}
