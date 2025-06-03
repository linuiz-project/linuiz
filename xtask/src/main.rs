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

    match <Arguments as clap::Parser>::parse() {
        Arguments::Build(build_options) => {
            build::build(&sh, build_options)?;
        }

        Arguments::Run(run_options) => {
            run::run(&sh, run_options)?;
        }
    }

    Ok(())
}
