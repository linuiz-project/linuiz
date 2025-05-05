static PARAMS: spin::Once<KernelParameters> = spin::Once::new();

#[derive(Debug, Clone, Copy)]
pub struct KernelParameters {
    /// Whether the kernel should utilize symmetric multi-processing.
    pub use_smp: bool,

    /// TODO
    pub keep_symbol_info: bool,

    /// Whether the kernel should use low-memory mode.
    pub low_memory_mode: bool,
}

impl Default for KernelParameters {
    fn default() -> Self {
        Self { use_smp: true, keep_symbol_info: false, low_memory_mode: false }
    }
}

pub fn parse(command_str: &str) {
    PARAMS.call_once(|| {
        let mut kernel_params = crate::params::KernelParameters::default();

        for arg in command_str.split(' ') {
            match arg {
                "--nosmp" => kernel_params.use_smp = false,
                "--symbolinfo" => kernel_params.keep_symbol_info = true,
                "--lomem" => kernel_params.low_memory_mode = true,

                // ignore
                "" => {}

                other => warn!("Unknown command line argument: {:?}", other),
            }
        }

        kernel_params
    });
}

pub fn use_smp() -> bool {
    PARAMS.get().unwrap().use_smp
}

pub fn keep_symbol_info() -> bool {
    PARAMS.get().unwrap().keep_symbol_info
}

pub fn use_low_memory() -> bool {
    PARAMS.get().unwrap().low_memory_mode
}
