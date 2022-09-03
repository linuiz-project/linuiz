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
            let shell = xshell::Shell::new()?;

            let _dir = shell.push_dir("src/");
            cmd!(shell, "cargo clean").run()
        }
    }
}
