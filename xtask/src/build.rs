use clap::{clap_derive::ArgEnum, Parser};
use std::path::PathBuf;
use xshell::{cmd, Shell};

#[allow(non_camel_case_types)]
#[derive(ArgEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    x64,
    rv64,
}

#[derive(Parser)]
pub struct Options {
    /// The compilation target for this build.
    #[clap(arg_enum, long)]
    target: Target,

    /// Whether the current build is a release build.
    #[clap(long)]
    release: bool,

    /// Whether to produce a disassembly file.
    #[clap(short, long)]
    disassemble: bool,

    /// Whether to output the result of `readelf` to a file.
    #[clap(short, long)]
    readelf: bool,

    /// Whether to use `cargo clippy` rather than `cargo build`.
    #[clap(short, long)]
    clippy: bool,
}

static REQUIRED_ROOT_DIRS: [&str; 5] = ["resources/", ".hdd/", ".hdd/root/EFI/BOOT/", ".hdd/root/linuiz/", ".debug/"];

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

pub fn build(options: Options) -> Result<(), xshell::Error> {
    let shell = Shell::new()?;

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
        let bootx64_efi_path = PathBuf::from("submodules/limine/BOOTX64.EFI");
        if !shell.path_exists(bootx64_efi_path.clone()) {
            cmd!(shell, "git submodule init").run()?;
        }

        // update the submodule to ensure latest version
        cmd!(shell, "git submodule update --recursive --remote").run()?;
        // copy the resultant EFI binary
        shell.copy_file(bootx64_efi_path.clone(), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
    }

    /* libkernel */
    {
        let _dir = shell.push_dir("libkernel/");
        cmd!(shell, "cargo fmt").run()?;
    }

    /* kernel */
    {
        let _dir = shell.push_dir("kernel/");
        let cargo_cmd_str = format!("{}", if options.clippy { "clippy" } else { "build" });
        let profile_str = format!("{}", if options.release { "release" } else { "dev" });
        let target_str = format!(
            "{}",
            match options.target {
                Target::x64 => "x86_64-unknown-none.json",
                Target::rv64 => "riscv64gc-unknown-none-elf",
            }
        );

        let _rustflags = shell.push_env(
            "RUSTFLAGS",
            format!(
                "-C link-arg=-T{}",
                match options.target {
                    Target::x64 => "x86_64-unknown-none.lds",
                    Target::rv64 => "riscv64gc-unknown-none.lds",
                }
            ),
        );

        cmd!(shell, "cargo fmt").run()?;
        cmd!(
            shell,
            "
                cargo {cargo_cmd_str}
                    --profile {profile_str}
                    --target {target_str}
                    -Z unstable-options
            "
        )
        .run()?;
    }

    let kernel_file_str = format!("kernel_{:?}.elf", options.target);

    // Copy kernel binary to root hdd
    shell.copy_file(
        PathBuf::from(
            format!(
                "kernel/target/{}/{}/kernel",
                // determine correct target path
                match options.target {
                    Target::x64 => "x86_64-unknown-none",
                    Target::rv64 => "riscv64gc-unknown-none-elf",
                },
                // determine correct build optimization
                if options.release { "release" } else { "debug" }
            )
            .to_lowercase(),
        ),
        PathBuf::from(format!(".hdd/root/linuiz/{}", kernel_file_str)),
    )?;

    /* disassemble kernel */
    if options.disassemble {
        match options.target {
            Target::x64 => {
                let output = cmd!(shell, "objdump -D .hdd/root/linuiz/{kernel_file_str}").output()?;
                shell.write_file(PathBuf::from(".debug/disassembly"), output.stdout)?;
            }
            Target::rv64 => panic!("`--disassemble` options cannot be used when targeting `rv64`"),
        }
    }

    if options.readelf {
        let output = cmd!(shell, "readelf -hlS .hdd/root/linuiz/{kernel_file_str}").output()?;
        shell.write_file(PathBuf::from(".debug/readelf"), output.stdout)?;
    }

    Ok(())
}
