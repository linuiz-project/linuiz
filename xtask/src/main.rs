pub mod build;
pub mod runner;

use clap::{AppSettings, Parser};
use xshell::cmd;

#[derive(Parser)]
#[clap(rename_all = "snake_case", setting = AppSettings::DisableVersionFlag)]
enum Arguments {
    Build(build::Options),
    Clippy(build::Options),
    Check(build::Options),
    Run(runner::Options),
    Clean,
    Metadata,
}

static CRATE_DIRS: [&str; 2] = ["src/kernel/", "src/drivers/"];

fn main() -> Result<(), xshell::Error> {
    match Arguments::parse() {
        Arguments::Build(build_options) => build::build(build_options),
        Arguments::Clippy(mut build_options) => build::build({
            build_options.clippy = true;
            build_options
        }),
        Arguments::Check(mut build_options) => build::build({
            build_options.check = true;
            build_options
        }),

        Arguments::Run(run_options) => runner::run(run_options),

        Arguments::Clean => {
            let shell = xshell::Shell::new()?;

            for crate_dir in CRATE_DIRS {
                let _dir = shell.push_dir(crate_dir);
                cmd!(shell, "cargo clean").run()?;
            }

            Ok(())
        }

        Arguments::Metadata => {
            let shell = xshell::Shell::new()?;

            for crate_dir in CRATE_DIRS {
                let _dir = shell.push_dir(crate_dir);
                cmd!(shell, "cargo metadata --format-version 1").run()?;
            }

            Ok(())
        }
    }
}
