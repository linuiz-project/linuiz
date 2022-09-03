pub mod build;
pub mod runner;

use clap::{AppSettings, Parser};
use xshell::cmd;

#[derive(Parser)]
#[clap(rename_all = "snake_case", setting = AppSettings::DisableVersionFlag)]
enum Arguments {
    Build(build::Options),
    Run(runner::Options),
    Clean,
}

fn main() -> Result<(), xshell::Error> {
    match Arguments::parse() {
        Arguments::Build(build_options) => build::build(build_options),
        Arguments::Run(run_options) => runner::run(run_options),
        Arguments::Clean => {
            static CLEAN_DIRS: [&str; 2] = ["src/kernel/", "src/drivers/"];

            let shell = xshell::Shell::new()?;

            for clean_dir in CLEAN_DIRS {
                let _dir = shell.push_dir(clean_dir);
                cmd!(shell, "cargo clean").run()?;
            }

            Ok(())
        }
    }
}
