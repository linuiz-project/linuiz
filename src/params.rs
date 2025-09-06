use crate::util::sync::Once;
use limine::request::ExecutableCmdlineRequest;

// TODO Add a parameter for choosing the physical memory allocation strategy.

#[derive(Debug)]
pub struct KernelParameters {
    /// Whether the kernel should utilize multi-processing.
    use_multiprocessing: bool,

    /// Whether to keep the kernel symbol info (for stack traces).
    drop_symbol_info: bool,

    /// Whether the kernel should use low-memory mode.
    low_memory_mode: bool,
}

static KERNEL_PARAMETERS: Once<KernelParameters> = Once::new();

impl Default for KernelParameters {
    fn default() -> Self {
        KernelParameters {
            use_multiprocessing: true,
            drop_symbol_info: false,
            low_memory_mode: false,
        }
    }
}

impl KernelParameters {
    pub fn init(kernel_cmdline_request: &ExecutableCmdlineRequest) {
        KERNEL_PARAMETERS.call_once(|| {
            let mut params = Self::default();

            let Some(kernel_cmdline_response) = kernel_cmdline_request.get_response() else {
                warn!("Bootloader did not provide response to kernel command line request.");
                return params;
            };

            let Ok(params_str) = kernel_cmdline_response.cmdline().to_str() else {
                warn!("Kernel command line contained invalid UTF-8.");
                return params;
            };

            params_str.split(' ').for_each(|param| {
                match param {
                    "" => {
                        // Ignore accidental extra spaces
                    }

                    "--no-multiprocessing" => params.use_multiprocessing = false,
                    "--drop-symbols" => params.drop_symbol_info = true,
                    "--low-memory" => params.low_memory_mode = true,

                    _ => {
                        warn!("Unknown kernel parameter: \"{param}\"");
                    }
                }
            });

            debug!("{params:#?}");

            params
        });
    }

    fn get_static() -> &'static Self {
        KERNEL_PARAMETERS.get().unwrap()
    }

    pub fn use_multiprocessing() -> bool {
        Self::get_static().use_multiprocessing
    }

    #[cfg(feature = "panic_traces")]
    pub fn drop_symbol_info() -> bool {
        Self::get_static().drop_symbol_info
    }

    pub fn use_low_memory() -> bool {
        Self::get_static().low_memory_mode
    }
}
