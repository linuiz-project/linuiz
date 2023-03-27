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

        let mut args = cmdline.split(' ');

        while let Some(arg) = args.next() {
            match arg {
                "--nosmp" => me.smp = false,
                "--symbolinfo" => me.symbolinfo = true,
                "--lomem" => me.low_memory = true,
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

pub static PARAMETERS: spin::Once<Parameters> = spin::Once::new();
