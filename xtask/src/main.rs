mod build;
mod run;

use clap::Parser;
use xshell::{cmd, Result, Shell};

#[derive(Debug, Clone, Copy, clap::Subcommand)]
#[allow(non_camel_case_types)]
pub enum Target {
    x86_64,
    riscv64gc,
}

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::x86_64 => "x86_64-unknown-none",
            Self::riscv64gc => "riscv64gc-unknown-none",
        })
    }
}

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Clean,
    Update,
    Check,

    #[command(subcommand)]
    Target(Target),

    Build(build::Options),
    Run(run::Options),
}

fn in_workspace_with(shell: &Shell, with_fn: impl Fn(&Shell) -> Result<()>) -> Result<()> {
    let _dir = shell.push_dir("src/");
    with_fn(shell)
}

fn main() -> Result<()> {
    let sh = Shell::new()?;

    match Arguments::parse() {
        Arguments::Clean => in_workspace_with(&sh, |sh| cmd!(sh, "cargo clean").run()),
        Arguments::Check => in_workspace_with(&sh, |sh| cmd!(sh, "cargo check --bins").run()),
        Arguments::Update => in_workspace_with(&sh, |sh| cmd!(sh, "cargo update").run())
            .and_then(|_| cmd!(sh, "git submodule update --init --recursive --remote").run()),

        Arguments::Target(target) => {
            let mut config = sh
                .read_file("src/.cargo/config.toml")?
                .parse::<toml_edit::Document>()
                .expect("invalid cargo config");
            config["build"]["target"] = toml_edit::value(target.to_string());
            sh.write_file("src/.cargo/config.toml", config.to_string())
        }

        Arguments::Build(build_options) => build::build(&sh, build_options),
        Arguments::Run(run_options) => run::run(&sh, run_options),
    }
}
