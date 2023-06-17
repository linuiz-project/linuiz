#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub smp: bool,
    pub symbolinfo: bool,
    pub low_memory: bool,
}

impl Parameters {
    pub fn parse(cmdline: &str) -> Self {
        let mut me = Self::default();

        if !cmdline.is_ascii() {
            warn!("Kernel command line must use ASCII characters only.");
            return me;
        }

        for arg in cmdline.split(' ') {
            match arg {
                "--nosmp" => me.smp = false,
                "--symbolinfo" => me.symbolinfo = true,
                "--lomem" => me.low_memory = true,

                // ignore
                "" => {}

                other => warn!("Unknown command line argument: {:?}", other),
            }
        }

        me
    }
}

impl Default for Parameters {
    fn default() -> Self {
        Self { smp: true, symbolinfo: false, low_memory: false }
    }
}

static PARAMETERS: spin::Once<Parameters> = spin::Once::new();

pub fn parse(cmdline: &str) {
    PARAMETERS.call_once(|| Parameters::parse(cmdline));
}

pub fn get() -> &'static Parameters {
    PARAMETERS.get().expect("parameters have not been parsed")
}
