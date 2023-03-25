
#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub smp: bool,
    pub symbolinfo: bool,
    pub low_memory: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { smp: true, symbolinfo: false, low_memory: false }
    }
}

pub static PARAMETERS: spin::Lazy<Parameters> = spin::Lazy::new(|| {
    crate::boot::get_kernel_file()
        .and_then(|kernel_file| kernel_file.cmdline.to_str())
        .and_then(|cmdline_cstr| cmdline_cstr.to_str().ok())
        .map(|cmdline| {
            let mut parameters = Parameters::default();

            for parameter in cmdline.split(' ') {
                match parameter.split_once(':') {
                    Some(("smp", "on")) => parameters.smp = true,
                    Some(("smp", "off")) => parameters.smp = false,

                    None if parameter == "symbolinfo" => parameters.symbolinfo = true,
                    None if parameter == "lomem" => parameters.low_memory = true,

                    _ => warn!("Unhandled cmdline parameter: {:?}", parameter),
                }
            }

            parameters
        })
        .unwrap_or_default()
});
