mod build;
mod run;

use clap::Parser;
use xshell::{cmd, Result, Shell};

#[derive(Debug, Clone, Copy, clap::Subcommand)]
#[allow(non_camel_case_types)]
pub enum Target {
    x86_64,
    riscv64,
}

impl Target {
    fn into_triple(self) -> &'static str {
        match self {
            Self::x86_64 => "x86_64-target.json",
            Self::riscv64 => "riscv64gc-unknown-none",
        }
    }
}

static WORKSPACE_DIRS: [&str; 3] = ["src/kernel/", "src/userspace/", "src/shared/"];
static BINARY_DIRS: [&str; 2] = ["src/kernel/", "src/userspace/"];

pub fn cargo_check(shell: &Shell, target: Target) -> Result<()> {
    BINARY_DIRS.iter().try_for_each(|path| {
        let _dir = shell.push_dir(path);
        let cargo_check = format!("cargo check --target {}", target.into_triple());
        cmd!(shell, "{cargo_check}").run()
    })
}

pub fn cargo_fmt(shell: &Shell) -> Result<()> {
    WORKSPACE_DIRS.iter().try_for_each(|path| {
        let _dir = shell.push_dir(path);
        cmd!(shell, "cargo fmt").run()
    })
}

pub fn cargo_clean(shell: &Shell) -> Result<()> {
    WORKSPACE_DIRS.iter().try_for_each(|path| {
        let _dir = shell.push_dir(path);
        cmd!(shell, "cargo clean").run()
    })
}

pub fn cargo_update(shell: &Shell) -> Result<()> {
    WORKSPACE_DIRS.iter().try_for_each(|path| {
        let _dir = shell.push_dir(path);
        cmd!(shell, "cargo update").run()
    })
}

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Clean,
    Update,

    #[command(subcommand)]
    Check(Target),

    Run(run::Options),
}

fn main() -> Result<()> {
    let shell = Shell::new()?;

    match Arguments::parse() {
        Arguments::Clean => cargo_clean(&shell),

        Arguments::Check(target) => cargo_check(&shell, target),

        Arguments::Update => {
            cmd!(shell, "git submodule update --init --recursive --remote").run()?;
            cargo_update(&shell)
        }

        Arguments::Run(run_options) => run::run(&shell, run_options),
    }
}
