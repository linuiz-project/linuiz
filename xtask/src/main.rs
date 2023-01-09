mod runner;
mod build;

use clap::Parser;
use xshell::{cmd, Result, Shell};

static WORKSPACE_DIRS: [&str; 3] = ["src/kernel/", "src/userspace/", "src/shared/"];

fn with_crate_dirs(shell: &Shell, mut with_fn: impl FnMut(&Shell) -> Result<()>) -> Result<()> {
    for crate_dir in WORKSPACE_DIRS {
        let _dir = shell.push_dir(crate_dir);
        with_fn(shell)?;
    }

    Ok(())
}

pub fn cargo_fmt(shell: &Shell) -> Result<()> {
    with_crate_dirs(shell, |shell| cmd!(shell, "cargo fmt").run())
}

pub fn cargo_check(shell: &Shell) -> Result<()> {
    with_crate_dirs(shell, |shell| cmd!(shell, "cargo check").run())
}

pub fn cargo_clean(shell: &Shell) -> Result<()> {
    with_crate_dirs(&shell, |shell| cmd!(shell, "cargo clean").run())
}



#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Check,
    Clean,
    Update,
    Run(runner::Options),
}

fn main() -> Result<()> {
    let shell = Shell::new()?;

    match Arguments::parse() {
        Arguments::Check => cargo_check(&shell),
        Arguments::Clean => cargo_clean(&shell),

        Arguments::Update => {
            cmd!(shell, "git submodule update --init --recursive --remote").run()?;
            with_crate_dirs(&shell, |shell| cmd!(shell, "cargo update").run())
        }

        Arguments::Run(run_options) => runner::run(&shell, run_options),
    }
}
