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
    ".hdd/root/linuiz/",   // kernel, drivers
];

pub fn build(shell: &Shell, options: Options) -> Result<()> {
    // Ensure root directories exist
    for root_dir in REQUIRED_ROOT_DIRS {
        if !shell.path_exists(root_dir) {
            shell.create_dir(root_dir)?;
        }
    }

    // Ensure dev disk image exists.
    if !shell.path_exists(".hdd/disk0.img") {
        cmd!(shell, "qemu-img create -f raw .hdd/disk0.img 256M").run()?;
    }

    // copy configuration to EFI image
    shell.copy_file("resources/limine.cfg", ".hdd/root/EFI/BOOT/")?;
    // copy the EFI binary image
    shell.copy_file("resources/limine/BOOTX64.EFI", ".hdd/root/EFI/BOOT/")?;

    cmd!(shell, "git submodule update --init --recursive --remote").run()?;

    // Configure rustc via the `RUSTFLAGS` environment variable.
    // let _rustflags = if !options.release {
    //     Some(shell.push_env("RUSTFLAGS", "-Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    // } else {
    //     None
    // };

    let _dir = shell.push_dir("src/");

    let tmp_dir_path = {
        let dir = shell.create_temp_dir()?;
        dir.path().to_owned()
    };
    let tmp_dir_path_str = tmp_dir_path.to_string_lossy();
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

    cmd!(shell, "cargo build --bins -Z unstable-options {cargo_args...}").run()?;

    shell.copy_file(&format!("{tmp_dir_path_str}/kernel"), "../.hdd/root/linuiz/")?;

    // compress userspace drivers and write to archive file
    let mut archive_builder = lza::ArchiveBuilder::new(options.compress.into());

    for path in shell.read_dir(&tmp_dir_path)?.into_iter() {
        let Some(file_name) = path.file_name().map(|name| name.to_string_lossy().into_owned())
                    else { continue };

        if options.drivers.contains(&file_name) {
            archive_builder
                .push_data(&file_name, shell.read_binary_file(path)?.as_slice())
                .expect("failed to write data to archive");
        }
    }

    let driver_data = archive_builder.take_data();
    println!("Compression resulted in a {} byte dump.", driver_data.len());
    shell.write_file("../.hdd/root/linuiz/drivers", driver_data)
}
