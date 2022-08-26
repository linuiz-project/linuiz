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
            static CLEAN_DIRS: [&str; 2] = ["kernel/", "libkernel/"];

            let shell = xshell::Shell::new()?;

            for dir_path in CLEAN_DIRS {
                let _dir = shell.push_dir(dir_path);
                cmd!(shell, "cargo clean").run()?;
            }

            Ok(())
        }
    }
}
