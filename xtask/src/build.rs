use clap::clap_derive::ValueEnum;
use lza::CompressionLevel;
use xshell::{cmd, Result, Shell};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Optimization {
    Fast,
    Small,
    All,
}

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

    #[clap(value_enum, short)]
    optimize: Option<Optimization>,
}

static REQUIRED_ROOT_DIRS: [&str; 3] = [
    ".hdd/",               // disk0.img
    ".hdd/root/EFI/BOOT/", // BOOTX64.EFI
    ".hdd/root/linuiz/",   // kernel, drivers
];

fn build_workspace<P: AsRef<std::path::Path>>(
    shell: &Shell,
    workspace_path: P,
    out_path: P,
    options: &Options,
) -> Result<()> {
    let out_path = out_path.as_ref().canonicalize().unwrap();

    let cargo_arguments = {
        let mut args = vec!["build", "-Z", "unstable-options", "--out-dir", out_path.to_str().unwrap()];

        args.push(if options.release {
            "--release"
        } else {
            // Only provide future-compatibiltiy notifications for development builds.
            "--future-incompat-report"
        });

        if options.verbose {
            args.push("-vv");
        }

        match options.optimize {
            Some(Optimization::Fast) => {
                args.extend(["--config", "opt-level=3", "--config", "lto=thin"]);
            }

            Some(Optimization::Small) => args.extend([
                "--config",
                "opt-level='z'",
                "--config",
                "codegen-units=1",
                "--config",
                "lto=fat",
                "--config",
                "strip=true",
            ]),

            Some(Optimization::All) => {
                args.extend([
                    "--config",
                    "opt-level=3",
                    "--config",
                    "codegen-units=1",
                    "--config",
                    "lto=fat",
                    "--config",
                    "strip=true",
                ]);
            }

            None => {}
        }

        args
    };

    let _dir = shell.push_dir(workspace_path);
    cmd!(shell, "cargo {cargo_arguments...}").run()
}

pub fn build(shell: &Shell, options: Options) -> Result<()> {
    cmd!(shell, "git submodule update --init --recursive --remote").run()?;

    // Configure rustc via the `RUSTFLAGS` environment variable.
    let _rustflags = if !options.release {
        Some(shell.push_env("RUSTFLAGS", "-Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    } else {
        None
    };

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

    // compile kernel
    build_workspace(shell, "src/kernel/", ".hdd/root/linuiz/", &options)?;

    // compile drivers
    let uncompressed_dir = shell.create_temp_dir()?;
    let uncompressed_path = uncompressed_dir.path().to_string_lossy().into_owned();
    build_workspace(shell, "src/userspace/", &uncompressed_path, &options)?;

    // compress userspace drivers and write to archive file
    let mut archive_builder = lza::ArchiveBuilder::new(options.compress.into());

    for path in shell.read_dir(uncompressed_path)?.into_iter() {
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
    shell.write_file(".hdd/root/linuiz/drivers", driver_data)
}
