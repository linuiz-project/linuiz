use clap::clap_derive::ValueEnum;
use lza::CompressionLevel;
use std::path::PathBuf;
use xshell::{cmd, Result, Shell};
use crate::Target;

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

static REQUIRED_ROOT_DIRS: [&str; 5] = ["resources/", ".hdd/", ".hdd/root/EFI/BOOT/", ".hdd/root/linuiz/", ".debug/"];

static LIMINE_DEFAULT_CFG: &str = "
    TIMEOUT=3

    :Linuiz (limine)
    COMMENT=Load Linuiz OS using the Stivale2 boot protocol.
    PROTOCOL=limine
    RESOLUTION=800x600x16
    KERNEL_PATH=boot:///linuiz/kernel.elf
    CMDLINE=smp:yes
    KASLR=yes
    ";

fn build_workspace(
    shell: &Shell,
    workspace_dir: PathBuf,
    out_path: PathBuf,
    target: Target,
    options: &Options,
) -> Result<()> {
    let out_path = out_path.canonicalize().unwrap();

    let cargo_arguments = {
        let mut args = vec!["build", "--target", target.as_ref(), "--out-dir", out_path.to_str().unwrap()];

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

    let _dir = shell.push_dir(workspace_dir);
    cmd!(shell, "cargo {cargo_arguments...}").run()
}

pub fn build(shell: &Shell, target: Target, options: Options) -> Result<()> {
    cmd!(shell, "git submodule update --init --recursive --remote").run()?;

    // Configure rustc via the `RUSTFLAGS` environment variable.
    let _rustflags = if !options.release {
        Some(shell.push_env("RUSTFLAGS", "-Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    } else {
        None
    };

    // Ensure root directories exist
    for root_dir in REQUIRED_ROOT_DIRS {
        let path = PathBuf::from(root_dir);
        if !shell.path_exists(&path) {
            shell.create_dir(path)?;
        }
    }

    // Ensure dev disk image exists.
    if !shell.path_exists(".hdd/disk0.img") {
        cmd!(shell, "qemu-img create -f raw .hdd/disk0.img 256M").run()?;
    }

    /* limine */
    {
        let limine_cfg_path = PathBuf::from("resources/limine.cfg");
        // create default configuration file if none are present
        if !shell.path_exists(limine_cfg_path.clone()) {
            shell.write_file(limine_cfg_path.clone(), LIMINE_DEFAULT_CFG)?;
        }
        // copy configuration to EFI image
        shell.copy_file(limine_cfg_path.clone(), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
        // copy the resultant EFI binary
        shell.copy_file(PathBuf::from("resources/limine/BOOTX64.EFI"), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
    }

    /* compile kernel */
    build_workspace(shell, PathBuf::from("src/kernel/"), PathBuf::from(".hdd/root/linuiz/"), target, &options)?;

    /* compile & compress drivers */
    static UNCOMPRESSED_DIR: &str = ".tmp/uncompressed/";
    build_workspace(shell, PathBuf::from("src/userspace/"), PathBuf::from(UNCOMPRESSED_DIR), target, &options)?;

    let mut archive_builder = lza::ArchiveBuilder::new(options.compress.into());

    for path in shell.read_dir(UNCOMPRESSED_DIR)?.into_iter() {
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
    shell.write_file(PathBuf::from(".hdd/root/linuiz/drivers"), driver_data)
}
