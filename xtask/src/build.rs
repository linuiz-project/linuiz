use clap::clap_derive::ValueEnum;
use lza::CompressionLevel;
use std::path::PathBuf;
use xshell::cmd;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Optimization {
    P,
    S,
    PS,
}

#[allow(non_camel_case_types)]
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    x64,
    rv64,
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
    /// The compilation target for this build.
    #[arg(value_enum, long, default_value = "x64")]
    arch: Architecture,

    /// Whether the current build is a release build.
    #[arg(long)]
    release: bool,

    /// Whether to produce a disassembly file.
    #[arg(short, long)]
    disassemble: bool,

    /// Whether to output the result of `readelf` to a file.
    #[arg(short, long)]
    readelf: bool,

    /// The compression level to use when compressing init device drivers.
    #[arg(value_enum, long, default_value = "default")]
    compress: Compression,

    /// Whether to use `cargo clippy` rather than `cargo build`.
    #[arg(short, long)]
    clippy: bool,

    /// Verbose build output. Equivalent to `cargo build -vv`.
    #[arg(short, long)]
    verbose: bool,

    /// Performs a `cargo clean` before building. This is useful for forcing a full recompile.
    #[arg(long)]
    clean: bool,

    /// Whether to force the compiler to *not* use `rbp` to store the stack frame pointer. This does nothing when compiling in release.
    #[arg(long)]
    no_stack_traces: bool,

    #[clap(value_enum, short)]
    optimize: Option<Optimization>,
}

static REQUIRED_ROOT_DIRS: [&str; 5] = ["resources/", ".hdd/", ".hdd/root/EFI/BOOT/", ".hdd/root/linuiz/", ".debug/"];
static PACKAGED_DRIVERS: [&str; 1] = ["test_driver"];

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

pub fn build(shell: &xshell::Shell, options: Options) -> Result<(), xshell::Error> {
    cmd!(shell, "git submodule update --init --recursive --remote").run()?;

    let workspace_root = shell.current_dir();

    // Configure rustc via the `RUSTFLAGS` environment variable.
    let _rustflags = if !options.release && !options.no_stack_traces {
        Some(shell.push_env("RUSTFLAGS", "-Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    } else {
        None
    };

    // Clean crates if required ...
    if options.clean {
        crate::clean(shell)?;
    }

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
        shell.copy_file(PathBuf::from("submodules/limine/BOOTX64.EFI"), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
    }

    fn disassemble(
        shell: &xshell::Shell,
        arch: Architecture,
        mut workspace_root: PathBuf,
        file_path: PathBuf,
    ) -> xshell::Result<()> {
        match arch {
            Architecture::x64 => {
                let output = cmd!(shell, "llvm-objdump -M intel -D {file_path}").output()?;
                let file_name = file_path.file_name().unwrap().to_str().unwrap();
                workspace_root.push(format!(".debug/disassembly_{file_name}"));
                shell.write_file(workspace_root, output.stdout)?;
            }
            Architecture::rv64 => panic!("`--disassemble` options cannot be used when targeting `rv64`"),
        }

        Ok(())
    }

    fn readelf(shell: &xshell::Shell, mut workspace_root: PathBuf, file_path: PathBuf) -> xshell::Result<()> {
        let output = cmd!(shell, "readelf -hlS {file_path}").output()?;
        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        workspace_root.push(format!(".debug/readelf_{file_name}"));
        shell.write_file(workspace_root, output.stdout)?;

        Ok(())
    }

    /* compile kernel */
    let profile_str = if options.release { "release" } else { "debug" };

    let cargo_arguments = {
        let mut args = vec![
            if options.clippy { "clippy" } else { "build" },
            "--profile",
            if options.release { "release" } else { "dev" },
        ];

        // Only provide future-compatibiltiy notifications for development builds.
        if !options.release {
            args.push("--future-incompat-report");
        }

        if options.verbose {
            args.push("-vv");
        }

        match options.optimize {
            Some(Optimization::P) => {
                args.push("--config");
                args.push("opt-level=3");

                args.push("--config");
                args.push("lto=thin");
            }

            Some(Optimization::S) => {
                args.push("--config");
                args.push("opt-level='z'");

                args.push("--config");
                args.push("codegen-units=1");

                args.push("--config");
                args.push("lto=fat");

                args.push("--config");
                args.push("strip=true");
            }

            Some(Optimization::PS) => {
                args.push("--config");
                args.push("opt-level=3");

                args.push("--config");
                args.push("codegen-units=1");

                args.push("--config");
                args.push("lto=fat");

                args.push("--config");
                args.push("strip=true");
            }

            None => {}
        }

        args.push("--target");

        args
    };

    // Compile kernel ...
    {
        {
            let _dir = shell.push_dir("src/kernel/");

            cmd!(shell, "cargo fmt").run()?;
            let mut cargo_arguments = cargo_arguments.clone();
            cargo_arguments.push(match options.arch {
                Architecture::x64 => "x86_64-linuiz-kernel.json",
                Architecture::rv64 => "riscv64gc-unknown-none-elf",
            });
            cmd!(shell, "cargo {cargo_arguments...}").run()?;
        }

        let out_path = PathBuf::from(".hdd/root/linuiz/kernel.elf");

        shell.copy_file(
            PathBuf::from(format!("src/kernel/target/x86_64-linuiz-kernel/{profile_str}/kernel")),
            PathBuf::from(out_path.clone()),
        )?;

        if options.disassemble {
            disassemble(&shell, options.arch, workspace_root.clone(), out_path.clone())?;
        }

        if options.readelf {
            readelf(&shell, workspace_root.clone(), out_path.clone())?;
        }
    }

    // Compile and compress drivers ...
    {
        let driver_data = {
            let _dir = shell.push_dir("src/userspace/");

            // Compile ...
            cmd!(shell, "cargo fmt").run()?;
            let mut cargo_arguments = cargo_arguments.clone();
            cargo_arguments.push(match options.arch {
                Architecture::x64 => "x86_64-unknown-linuiz.json",
                Architecture::rv64 => "riscv64gc-unknown-none-elf",
            });
            cmd!(shell, "cargo {cargo_arguments...}").run()?;

            // Compress ...
            let mut archive_builder = lza::ArchiveBuilder::new(options.compress.into());

            for driver_name in PACKAGED_DRIVERS {
                let driver_path = PathBuf::from(format!("target/x86_64-unknown-linuiz/{profile_str}/{driver_name}"));

                // Compress and append driver bytes.
                let (header, data) = archive_builder
                    .push_data(driver_name, shell.read_binary_file(driver_path.clone())?.as_slice())
                    .expect("failed to write data to archive");

                if !options.release {
                    println!("{:?}\nData Snippet: {:?}", header, &data[..100]);
                }

                if options.disassemble {
                    disassemble(&shell, options.arch, workspace_root.clone(), driver_path.clone())?;
                }

                if options.readelf {
                    readelf(&shell, workspace_root.clone(), driver_path.clone())?;
                }
            }

            archive_builder.take_data()
        };

        println!("Compression resulted in a {} byte dump.", driver_data.len());
        shell.write_file(PathBuf::from(".hdd/root/linuiz/drivers"), driver_data)?;
    }

    Ok(())
}
