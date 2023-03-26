use std::{fs::File, io::Error, path::PathBuf};

use clap::clap_derive::ValueEnum;
use lza::CompressionLevel;
use xshell::{cmd, Result, Shell};

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum Compression {
    None,
    Fast,
    Small,
    Smallest,
    Default,
}

impl Into<CompressionLevel> for Compression {
    fn into(self) -> CompressionLevel {
        match self {
            Compression::None => CompressionLevel::NoCompression,
            Compression::Fast => CompressionLevel::BestSpeed,
            Compression::Small => CompressionLevel::BestCompression,
            Compression::Smallest => CompressionLevel::UberCompression,
            Compression::Default => CompressionLevel::DefaultLevel,
        }
    }
}

#[derive(clap::Parser)]
#[allow(non_snake_case)]
pub struct Options {
    /// Whether the current build is a release build.
    #[arg(long)]
    release: bool,

    /// The compression level to use when compressing init device drivers.
    #[arg(value_enum, long, default_value = "default")]
    compress: Compression,

    /// Verbose build output. Equivalent to `cargo build -vv`.
    #[arg(short, long)]
    verbose: bool,

    #[arg(long, default_value = "test_driver")]
    drivers: Vec<String>,
}

static REQUIRED_ROOT_DIRS: [&str; 3] = [
    ".hdd/",               // disk0.img
    ".hdd/root/EFI/BOOT/", // BOOTX64.EFI
    ".hdd/root/pyre/",     // kernel, drivers
];

pub fn build(sh: &Shell, options: Options) -> Result<()> {
    // Ensure root directories exist
    for root_dir in REQUIRED_ROOT_DIRS {
        if !sh.path_exists(root_dir) {
            sh.create_dir(root_dir)?;
        }
    }

    // Ensure dev disk image exists.
    if !sh.path_exists(".hdd/disk0.img") {
        cmd!(sh, "qemu-img create -f raw .hdd/disk0.img 256M").run()?;
    }

    // copy configuration to EFI image
    sh.copy_file("resources/limine.cfg", ".hdd/root/EFI/BOOT/")?;
    // copy the EFI binary image
    sh.copy_file("resources/limine/BOOTX64.EFI", ".hdd/root/EFI/BOOT/")?;

    cmd!(sh, "git submodule update --init --recursive --remote").run()?;

    // Configure rustc via the `RUSTFLAGS` environment variable.
    // let _rustflags = if !options.release {
    //     Some(shell.push_env("RUSTFLAGS", "-Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    // } else {
    //     None
    // };

    let root_dir = sh.current_dir();
    let _dir = sh.push_dir("src/");
    let tmp_dir = sh.create_temp_dir()?;
    let tmp_dir_path_str = tmp_dir.path().to_string_lossy();

    let cargo_args = {
        let mut args = vec!["--out-dir", &tmp_dir_path_str];

        args.push({
            if options.release {
                "--release"
            } else {
                // Only provide future-compatibiltiy notifications for development builds.
                "--future-incompat-report"
            }
        });

        if options.verbose {
            args.push("-vv");
        }

        args
    };

    cmd!(sh, "cargo build --bins -Z unstable-options {cargo_args...}").run()?;
    // Copy the output kernel binary to the virtual HDD.
    sh.copy_file(&format!("{tmp_dir_path_str}/pyre"), root_dir.join(".hdd/root/pyre/kernel"))?;

    let archive_file =
        File::create(&format!("{tmp_dir_path_str}/drivers")).expect("failed to create or open the driver package file");
    build_drivers_archive(archive_file, &options.drivers, sh.read_dir(&*tmp_dir_path_str)?.into_iter())
        .expect("error attempting to package drivers");

    Ok(())
}

fn build_drivers_archive(
    archive_file: File,
    include_drivers: &[String],
    files: impl Iterator<Item = PathBuf>,
) -> Result<(), Error> {
    // compress userspace drivers and write to archive file
    let mut archive_builder = tar::Builder::new(archive_file);
    files
        // Filter out any drivers that don't need to be included.
        .filter(|path| {
            path.file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .filter(|driver_name| include_drivers.iter().any(|s| s.eq(driver_name)))
                .is_some()
        })
        // Attempt to package & write the drivers to the tar archive on disk.
        .try_for_each(|path| archive_builder.append_file(&path, &mut File::open(&path)?))?;

    archive_builder.finish()
}
