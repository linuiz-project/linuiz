pub mod build;
pub mod runner;

use std::path::PathBuf;

use clap::Parser;
use xshell::cmd;

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Build(build::Options),
    Run(runner::Options),
    Clean,
    Metadata,
    Update,
}

fn main() -> Result<(), xshell::Error> {
    let shell = xshell::Shell::new()?;

    match Arguments::parse() {
        Arguments::Build(build_options) => {
            keep_submodules_updated(&shell)?;
            build::build(&shell, build_options)
        }

        Arguments::Run(run_options) => runner::run(&shell, run_options),

        Arguments::Clean => clean(&shell),

        Arguments::Update => {
            keep_submodules_updated(&shell)?;
            update(&shell)
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

static CRATE_DIRS: [&str; 2] = ["src/kernel/", "src/userspace/"];

pub fn clean(shell: &xshell::Shell) -> xshell::Result<()> {
    for crate_dir in CRATE_DIRS {
        let _dir = shell.push_dir(crate_dir);
        cmd!(shell, "cargo clean").run()?;
    }

    Ok(())
}

pub fn update(shell: &xshell::Shell) -> xshell::Result<()> {
    for crate_dir in CRATE_DIRS {
        let _dir = shell.push_dir(crate_dir);
        cmd!(shell, "cargo update").run()?;
    }

    Ok(())
}

fn keep_submodules_updated(shell: &xshell::Shell) -> xshell::Result<()> {
    use std::path::PathBuf;

    static SUBMODULES: [&str; 4] = ["limine", "lza", "try_alloc", "spin-rs"];

    for submodule_name in SUBMODULES {
        if !PathBuf::from(format!("submodules/{}", submodule_name)).exists() {
            cmd!(shell, "git submodule init").run()?;
            break;
        }
    }

    Ok(())
}
