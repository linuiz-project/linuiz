pub mod build;
pub mod runner;

use clap::Parser;
use xshell::cmd;

static CRATE_DIRS: [&str; 3] = ["src/kernel/", "src/userspace/", "src/shared/"];

fn with_crate_dirs(
    shell: &xshell::Shell,
    mut with_fn: impl FnMut(&xshell::Shell) -> xshell::Result<()>,
) -> xshell::Result<()> {
    for crate_dir in CRATE_DIRS {
        let _dir = shell.push_dir(crate_dir);
        with_fn(shell)?;
    }

    Ok(())
}

pub fn cargo_fmt(shell: &xshell::Shell) -> xshell::Result<()> {
    with_crate_dirs(shell, |shell| cmd!(shell, "cargo fmt").run())
}

pub fn cargo_clean(shell: &xshell::Shell) -> xshell::Result<()> {
    with_crate_dirs(shell, |shell| cmd!(shell, "cargo clean").run())
}

pub fn cargo_update(shell: &xshell::Shell) -> xshell::Result<()> {
    with_crate_dirs(shell, |shell| cmd!(shell, "cargo update").run())
}

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Build(build::Options),
    Run(runner::Options),
    Clean,
    Metadata,
    Update,
}

fn main() -> xshell::Result<()> {
    let shell = xshell::Shell::new()?;

    match Arguments::parse() {
        Arguments::Build(build_options) => {
            cargo_fmt(&shell)?;
            build::build(&shell, build_options)
        }

        Arguments::Run(run_options) => runner::run(&shell, run_options),

        Arguments::Clean => cargo_clean(&shell),

        Arguments::Update => {
            cmd!(shell, "git submodule update --init --recursive --remote").run()?;
            cargo_update(&shell)
        }

        Arguments::Metadata => {
            for crate_dir in CRATE_DIRS {
                let _dir = shell.push_dir(crate_dir);
                cmd!(shell, "cargo metadata --format-version 1").run()?;
            }

            Ok(())
        }
    }
}
