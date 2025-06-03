use anyhow::Result;
use std::{fs::File, io::Error, path::Path};
use xshell::{Shell, cmd};

#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Target {
    x86_64,
    riscv64gc,
    aarch64,
}

impl Target {
    pub const fn as_triple(&self) -> &'static str {
        match self {
            Target::x86_64 => "x86_64-unknown-none",
            Target::riscv64gc => unimplemented!(),
            Target::aarch64 => unimplemented!(),
        }
    }
}

#[derive(Parser)]
pub struct Options {
    /// Whether to build in release mode (with all optimizations).
    #[arg(long)]
    release: bool,

    /// Verbose build output. Equivalent to `cargo build -vv`.
    #[arg(short, long)]
    verbose: bool,

    /// Whether to print the kernel's build fingerprint.
    /// This can be useful for debugging constant rebuilds.
    #[arg(long)]
    fingerprint: bool,

    /// Target platform to compile for.
    #[arg(short, long)]
    target: Target,

    #[arg(long)]
    drivers: Vec<String>,
}

pub fn build(sh: &Shell, options: Options) -> Result<()> {
    let _cargo_log = {
        let mut cargo_log = Vec::new();

        if options.fingerprint {
            cargo_log.push("cargo::core::compiler::fingerprint=info");
        }

        sh.push_env("CARGO_LOG", cargo_log.join(" "))
    };

    let root_dir = sh.current_dir();

    let tmp_dir = sh.create_temp_dir()?;
    let tmp_dir_path = tmp_dir.path();

    cmd!(sh, "cargo fmt --check").run()?;

    let mut build_cmd = cmd!(sh, "cargo build")
        .args(["--target", options.target.as_triple()])
        .args(["--artifact-dir", tmp_dir_path.to_str().unwrap()])
        .args(["-Z", "unstable-options"]);

    if options.release {
        build_cmd = build_cmd.arg("--release");
    } else {
        // Only provide future-compatibiltiy notifications for development builds.
        build_cmd = build_cmd.arg("--future-incompat-report")
    }

    if options.verbose {
        build_cmd = build_cmd.arg("-vv")
    }

    build_cmd.run()?;

    if !sh.path_exists("run/system/linuiz") {
        sh.create_dir("run/system/linuiz")?;
    }

    // Copy the kernel binary to the virtual HDD.
    sh.copy_file(
        tmp_dir_path.join("kernel"),
        root_dir.join("run/system/linuiz/kernel"),
    )?;

    build_drivers_archive(
        tmp_dir_path,
        root_dir.join("run/system/linuiz/drivers"),
        sh.read_dir(tmp_dir_path)?.into_iter(),
        &options.drivers,
    )
    .expect("error attempting to package drivers");

    Ok(())
}

fn build_drivers_archive<P1: AsRef<Path>, P2: AsRef<Path>>(
    drivers_path: P1,
    archive_path: P2,
    files: impl Iterator<Item = std::path::PathBuf>,
    include_drivers: &[String],
) -> Result<(), Error> {
    let drivers_path = drivers_path.as_ref();

    // compress userspace drivers and write to archive file
    let mut archive_builder = tar::Builder::new(
        File::create(archive_path).expect("failed to create or open the driver package file"),
    );

    files
        .filter(|p| {
            p.file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .filter(|driver_name| include_drivers.iter().any(|s| s.eq(driver_name)))
                .is_some()
        })
        .try_for_each(|path| {
            println!("Packaging driver: {:?}", path.file_name().unwrap());

            let rel_path = path.strip_prefix(drivers_path).unwrap();
            archive_builder.append_file(rel_path, &mut File::open(&path)?)
        })?;

    archive_builder.finish()
}
