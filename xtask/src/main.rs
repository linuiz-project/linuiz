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
