use limine::request::ExecutableCmdlineRequest;

static PARAMS: spin::RwLock<KernelParameters> =
    spin::RwLock::new(KernelParameters { use_multiprocessing: true, keep_symbol_info: false, low_memory_mode: false });

#[derive(Debug, Clone, Copy)]
pub struct KernelParameters {
    /// Whether the kernel should utilize multi-processing.
    pub use_multiprocessing: bool,

    /// Whether to keep the kernel symbol info before reclaiming extra memory.
    pub keep_symbol_info: bool,

    /// Whether the kernel should use low-memory mode.
    pub low_memory_mode: bool,
}

// TODO figure out if we're segregating the Limine request for this function
// pub fn parse_cmdline() {
//     static KERNEL_CMDLINE_REQUEST: BootOnly<ExecutableCmdlineRequest> = BootOnly::new(ExecutableCmdlineRequest::new());

//     let mut params = PARAMS.write();

//     if let Some(response) = KERNEL_CMDLINE_REQUEST.get().get_response() {
//         for arg in response.cmdline().to_str().expect("kernel command string is not valid UTF-8").split(' ') {
//             match arg {
//                 "--nomp" => params.use_multiprocessing = false,
//                 "--symbolinfo" => params.keep_symbol_info = true,
//                 "--lomem" => params.low_memory_mode = true,

//                 other => warn!("Unknown command line argument: {other:?}"),
//             }
//         }
//     } else {
//         info!("No kernel cmdline provided.");
//     }

//     info!("Parameters:\n{:?}", *params);
// }

pub fn use_multiprocessing() -> bool {
    PARAMS.read().use_multiprocessing
}

pub fn keep_symbol_info() -> bool {
    PARAMS.read().keep_symbol_info
}

pub fn use_low_memory() -> bool {
    PARAMS.read().low_memory_mode
}
