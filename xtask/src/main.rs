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

pub fn cargo_check(shell: &Shell) -> Result<()> {
    BINARY_DIRS.iter().try_for_each(|path| {
        let _dir = shell.push_dir(path);
        cmd!(shell, "cargo check --bins").run()
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
    Check,

    #[command(subcommand)]
    Target(Target),

    Run(run::Options),
}

fn main() -> Result<()> {
    let shell = Shell::new()?;

    match Arguments::parse() {
        Arguments::Clean => cargo_clean(&shell),
        Arguments::Check => cargo_check(&shell),

        Arguments::Target(target) => WORKSPACE_DIRS.iter().try_for_each(|path| {
            let _dir = shell.push_dir(path);

            let Ok(cargo_config) = shell.read_file(".cargo/config.toml")
                else {
                    return Ok(())
                };

            let mut config = cargo_config.parse::<toml_edit::Document>().expect("invalid toml for cargo config");
            config["build"]["target"] = toml_edit::value(target.into_triple());
            shell.write_file(".cargo/config.toml", config.to_string())
        }),

        Arguments::Update => {
            cmd!(shell, "git submodule update --init --recursive --remote").run()?;
            cargo_update(&shell)
        }

        Arguments::Run(run_options) => run::run(&shell, run_options),
    }
}
