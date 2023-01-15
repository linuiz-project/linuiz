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

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::x86_64 => "x86_64-target.json",
            Self::riscv64 => "riscv64gc-unknown-none",
        })
    }
}

fn get_target(shell: &Shell) -> Result<String> {
    shell.read_file("targets/.target")
}

fn with_workspace_dirs(shell: &Shell, with_fn: impl Fn() -> Result<()>) -> Result<()> {
    shell.read_dir("src/")?.into_iter().try_for_each(|path| {
        shell.read_dir(path.clone()).and_then(|_| {
            let _dir = shell.push_dir(path.clone());
            with_fn()
        })
    })
}

pub fn check(shell: &Shell) -> Result<()> {
    let target = get_target(shell)?;
    with_workspace_dirs(shell, || cmd!(shell, "cargo check --bins --target {target}").run())
}

pub fn cargo_fmt(shell: &Shell) -> Result<()> {
    with_workspace_dirs(shell, || cmd!(shell, "cargo fmt").run())
}

pub fn clean(shell: &Shell) -> Result<()> {
    with_workspace_dirs(shell, || cmd!(shell, "cargo clean").run())
}

pub fn update(shell: &Shell) -> Result<()> {
    cmd!(shell, "git submodule update --init --recursive --remote").run()?;
    with_workspace_dirs(shell, || cmd!(shell, "cargo update").run())
}

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Clean,
    Update,
    Check,

    #[command(subcommand)]
    Target(Target),

    Run(run::Options),
}

fn main() -> Result<()> {
    let shell = Shell::new()?;

    match Arguments::parse() {
        Arguments::Clean => clean(&shell),
        Arguments::Check => check(&shell),
        Arguments::Update => update(&shell),

        Arguments::Target(target) => shell.write_file("targets/.target", target.to_string()),

        Arguments::Run(run_options) => run::run(&shell, run_options),
    }
}
