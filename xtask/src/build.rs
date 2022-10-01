use clap::clap_derive::ValueEnum;
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

impl Compression {
    fn as_u8(&self) -> u8 {
        match self {
            Compression::None => 0,
            Compression::Fast => 1,
            Compression::Small => 9,
            Compression::Smallest => 10,
            Compression::Default => 6,
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

        // initialize git submodule if it hasn't been
        let bootx64_efi_path = PathBuf::from("resources/submodules/limine/BOOTX64.EFI");
        if !shell.path_exists(bootx64_efi_path.clone()) {
            cmd!(shell, "git submodule init").run()?;
        }

        // update the submodule to ensure latest version
        cmd!(shell, "git submodule update --recursive --remote").run()?;
        // copy the resultant EFI binary
        shell.copy_file(bootx64_efi_path.clone(), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
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
        let mut vec = vec![
            if options.clippy { "clippy" } else { "build" },
            "--profile",
            if options.release { "release" } else { "dev" },
            "--future-incompat-report",
        ];

        if options.verbose {
            vec.push("-vv");
        }

        match options.optimize {
            Some(Optimization::P) => {
                vec.push("--config");
                vec.push("opt-level=3");

                vec.push("--config");
                vec.push("lto=thin");
            }

            Some(Optimization::S) => {
                vec.push("--config");
                vec.push("opt-level='z'");

                vec.push("--config");
                vec.push("codegen-units=1");

                vec.push("--config");
                vec.push("lto=fat");

                vec.push("--config");
                vec.push("strip=true");
            }

            Some(Optimization::PS) => {
                vec.push("--config");
                vec.push("opt-level=3");

                vec.push("--config");
                vec.push("codegen-units=1");

                vec.push("--config");
                vec.push("lto=fat");

                vec.push("--config");
                vec.push("strip=true");
            }

            None => {}
        }

        vec.push("--target");

        vec
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
        // TODO I'm not sure how I feel about the layout of this whole block.
        let compressed_drivers = {
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
            let mut bytes = vec![];

            for driver_name in PACKAGED_DRIVERS {
                let driver_path = PathBuf::from(format!("target/x86_64-unknown-linuiz/{profile_str}/{driver_name}"));

                // Compress and append driver bytes.
                {
                    let file_bytes = shell.read_binary_file(driver_path.clone())?;
                    let mut compressed_bytes =
                        miniz_oxide::deflate::compress_to_vec(&file_bytes, options.compress.as_u8());

                    println!(
                        "Compress driver '{}': {} -> {} bytes",
                        driver_name,
                        file_bytes.len(),
                        compressed_bytes.len()
                    );

                    let bytes_len = compressed_bytes.len();
                    bytes.push((bytes_len >> 0) as u8);
                    bytes.push((bytes_len >> 8) as u8);
                    bytes.push((bytes_len >> 16) as u8);
                    bytes.push((bytes_len >> 24) as u8);
                    bytes.push((bytes_len >> 32) as u8);
                    bytes.push((bytes_len >> 40) as u8);
                    bytes.push((bytes_len >> 48) as u8);
                    bytes.push((bytes_len >> 56) as u8);
                    bytes.append(&mut compressed_bytes);
                }

                if options.disassemble {
                    disassemble(&shell, options.arch, workspace_root.clone(), driver_path.clone())?;
                }

                if options.readelf {
                    readelf(&shell, workspace_root.clone(), driver_path.clone())?;
                }
            }

            bytes
        };

        println!("Compression resulted in a {} byte dump.", compressed_drivers.len());
        shell.write_file(PathBuf::from(".hdd/root/linuiz/drivers"), compressed_drivers)?;
    }

    Ok(())
}
