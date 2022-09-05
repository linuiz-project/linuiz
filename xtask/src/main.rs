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
    Metadata,
}

fn main() -> Result<(), xshell::Error> {
    let shell = xshell::Shell::new()?;

    match Arguments::parse() {
        Arguments::Build(build_options) => build::build(&shell, build_options),

        Arguments::Run(run_options) => runner::run(&shell, run_options),

        Arguments::Clean => clean(&shell),

        Arguments::Metadata => {
            for crate_dir in CRATE_DIRS {
                let _dir = shell.push_dir(crate_dir);
                cmd!(shell, "cargo metadata --format-version 1").run()?;
            }

            Ok(())
        }
    }
}

static CRATE_DIRS: [&str; 2] = ["src/kernel/", "src/drivers/"];

pub fn clean(shell: &xshell::Shell) -> xshell::Result<()> {
    for crate_dir in CRATE_DIRS {
        let _dir = shell.push_dir(crate_dir);
        cmd!(shell, "cargo clean").run()?;
    }

    Ok(())
}
