mod build;
mod run;

use clap::Parser;
use xshell::{cmd, Result, Shell};

#[derive(Debug, Clone, Copy, clap::Subcommand)]
#[allow(non_camel_case_types)]
pub enum Target {
    x86_64,
    RV64,
}

impl AsRef<str> for Target {
    fn as_ref(&self) -> &str {
        match self {
            Self::x86_64 => "x86_64-target.json",
            Self::RV64 => "riscv64gc-unknown-none",
        }
    }
}

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
    Run(run::Options),
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

        Arguments::Run(run_options) => run::run(&shell, run_options),
    }
}
