mod build;
mod run;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate xshell;

#[derive(Parser)]
enum Arguments {
    Build(build::Options),
    Run(run::Options),
}

fn main() -> anyhow::Result<()> {
    let sh = xshell::Shell::new()?;

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
