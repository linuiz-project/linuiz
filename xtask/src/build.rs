use clap::{clap_derive::ArgEnum, Parser};
use std::path::PathBuf;
use xshell::cmd;

#[allow(non_camel_case_types)]
#[derive(ArgEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    x64,
    rv64,
}

#[derive(ArgEnum, Debug, Clone, Copy)]
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

#[derive(Parser)]
pub struct Options {
    /// The compilation target for this build.
    #[clap(arg_enum, long, default_value = "x64")]
    arch: Architecture,

    /// Whether the current build is a release build.
    #[clap(long)]
    release: bool,

    /// Whether to produce a disassembly file.
    #[clap(short, long)]
    disassemble: bool,

    /// Whether to output the result of `readelf` to a file.
    #[clap(short, long)]
    readelf: bool,

    /// The compression level to use when compressing init device drivers.
    #[clap(arg_enum, long, default_value = "default")]
    compress: Compression,

    /// Whether to use `cargo clippy` rather than `cargo build`.
    #[clap(short, long)]
    clippy: bool,

    /// Whether to force the compiler to use `rbp` to store the stack frame pointer. This allows semantic stack tracing.
    #[clap(long)]
    no_stack_traces: bool,
}

static REQUIRED_ROOT_DIRS: [&str; 5] = ["resources/", ".hdd/", ".hdd/root/EFI/BOOT/", ".hdd/root/linuiz/", ".debug/"];
static PACKAGED_DRIVERS: [&str; 1] = ["test_driver"];

static LIMINE_DEFAULT_CFG: &str = "
    TIMEOUT=3

    :Linuiz (limine)
    COMMENT=Load Linuiz OS using the Stivale2 boot protocol.
    PROTOCOL=limine
    RESOLUTION=800x600x16
    KERNEL_PATH=boot:///linuiz/kernel_x64.elf
    CMDLINE=smp:yes
    KASLR=yes
    ";

pub fn build(shell: &xshell::Shell, options: Options) -> Result<(), xshell::Error> {
    let workspace_root = shell.current_dir();

    let _rustflags = if !options.release && !options.no_stack_traces {
        Some(shell.push_env("RUSTFLAGS", "-Cforce-unwind-tables -Cforce-frame-pointers -Csymbol-mangling-version=v0"))
    } else {
        None
    };

    /* setup default files and folders */
    {
        for root_dir in REQUIRED_ROOT_DIRS {
            let path = PathBuf::from(root_dir);
            if !shell.path_exists(&path) {
                shell.create_dir(path)?;
            }
        }

        if !shell.path_exists(".hdd/disk0.img") {
            cmd!(shell, "qemu-img create -f raw .hdd/disk0.img 256M").run()?;
        }
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

    /* compile kernel */
    let profile_str = if options.release { "release" } else { "debug" };

    let cargo_arguments = vec![
        if options.clippy { "clippy" } else { "build" },
        "--profile",
        if options.release { "release" } else { "dev" },
        "--target",
    ];

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
                workspace_root.push(format!(".debug/disassembly_{}", file_name));
                shell.write_file(workspace_root, output.stdout)?;
            }
            Architecture::rv64 => panic!("`--disassemble` options cannot be used when targeting `rv64`"),
        }

        Ok(())
    }

    fn readelf(shell: &xshell::Shell, mut workspace_root: PathBuf, file_path: PathBuf) -> xshell::Result<()> {
        let output = cmd!(shell, "readelf -hlS {file_path}").output()?;
        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        workspace_root.push(format!(".debug/readelf_{}", file_name));
        shell.write_file(workspace_root, output.stdout)?;

        Ok(())
    }

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
            PathBuf::from(format!("src/kernel/target/x86_64-linuiz-kernel/{}/kernel", profile_str)),
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
        let compressed_drivers = {
            let _dir = shell.push_dir("src/drivers/");

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
                let driver_path =
                    PathBuf::from(format!("target/x86_64-unknown-linuiz/{}/{}", profile_str, driver_name));
                let mut file_bytes = shell.read_binary_file(driver_path.clone())?;

                let byte_offset = file_bytes.len();

                bytes.push((byte_offset >> 0) as u8);
                bytes.push((byte_offset >> 8) as u8);
                bytes.push((byte_offset >> 16) as u8);
                bytes.push((byte_offset >> 24) as u8);
                bytes.push((byte_offset >> 32) as u8);
                bytes.push((byte_offset >> 40) as u8);
                bytes.push((byte_offset >> 48) as u8);
                bytes.push((byte_offset >> 56) as u8);

                bytes.append(&mut file_bytes);

                if options.disassemble {
                    disassemble(&shell, options.arch, workspace_root.clone(), driver_path.clone())?;
                }

                if options.readelf {
                    readelf(&shell, workspace_root.clone(), driver_path.clone())?;
                }
            }

            println!("Compressing {} bytes of driver files...", bytes.len());
            miniz_oxide::deflate::compress_to_vec(&bytes, options.compress.as_u8())
        };

        println!("Compression resulted in a {} byte dump.", compressed_drivers.len());
        shell.write_file(PathBuf::from(".hdd/root/linuiz/drivers"), compressed_drivers)?;
    }

    Ok(())
}
