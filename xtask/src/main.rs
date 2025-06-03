mod build;
mod run;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate xshell;

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Build(build::Options),
    Run(run::Options),
}

fn main() -> anyhow::Result<()> {
    let sh = xshell::Shell::new()?;

    // Ensure there's a debug directory for logs or the like.
    if !sh.path_exists(".debug/") {
        sh.create_dir(".debug/")?;
    }

    // Ensure development disk image exists.
    if !sh.path_exists("run/disk0.img") {
        cmd!(sh, "qemu-img create -f raw run/disk0.img 256M").run()?;
    }

    let temp_dir = sh.create_temp_dir()?;

    match <Arguments as clap::Parser>::parse() {
        Arguments::Build(build_options) => {
            build::build(&sh, temp_dir.path(), build_options)?;
        }

        Arguments::Run(run_options) => {
            run::run(&sh, temp_dir.path(), run_options)?;
        }
    }

    Ok(())
}
