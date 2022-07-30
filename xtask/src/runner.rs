use crate::{BlockDriver, Simulator};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Options {
    /// Number of CPUs to emulate.
    #[clap(long, default_value = "4")]
    cpus: usize,

    // RAM size in MB.
    #[clap(long, default_value = "512")]
    ram: usize,

    /// Enables debug logging to the specified location.
    #[clap(long)]
    debug_log: Option<PathBuf>,

    /// Which simulator to use when executing the binary.
    #[clap(arg_enum, long, default_value = "kvm")]
    simulator: Simulator,

    /// Which type of block driver to use for root drive.
    #[clap(arg_enum, long)]
    block_driver: BlockDriver,
}

pub fn run(options: Options) -> Result<(), xshell::Error> {
    Ok(())
}
