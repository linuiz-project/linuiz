use clap::{clap_derive::ArgEnum, Parser};
use std::path::PathBuf;
use xshell::{cmd, Shell};

#[derive(Debug, Clone, Copy, ArgEnum)]
pub enum Profile {
    Release,
    Debug,
}

#[derive(Parser)]
pub struct Options {
    /// Whether the current build is debug mode or not.
    #[clap(arg_enum, long, default_value = "debug")]
    profile: Profile,

    /// Whether to produce a disassembly file.
    #[clap(long)]
    disassemble: bool,

    // Whether to output the result of `readelf` to a file.
    #[clap(long)]
    readelf: bool,
}

static REQUIRED_ROOT_DIRS: [&str; 5] = ["resources/", ".hdd/", ".hdd/root/EFI/BOOT/", ".hdd/root/linuiz/", ".debug/"];

static LIMINE_DEFAULT_CFG: &str = "
    TIMEOUT=3

    :Linuiz (limine)
    COMMENT=Load Linuiz OS using the Stivale2 boot protocol.
    PROTOCOL=limine
    RESOLUTION=800x600x16
    KERNEL_PATH=boot:///linuiz/kernel.elf
    KASLR=no
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

    /* build */
    {
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
            cmd!(shell, "git submodule update").run()?;
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
            {
                let _dir = shell.push_dir("kernel/");
                let profile_str = format!(
                    "{}",
                    match options.profile {
                        Profile::Release => "release",
                        Profile::Debug => "dev",
                    }
                );

                cmd!(shell, "cargo fmt").run()?;
                cmd!(
                    shell,
                    "
                    cargo build
                        --profile {profile_str}
                        --target x86_64-unknown-none.json
                        -Z unstable-options
                    "
                )
                .run()?;
            }

            // Copy kernel binary to root hdd
            shell.copy_file(
                PathBuf::from(
                    format!("kernel/target/x86_64-unknown-none/{:?}/kernel.elf", options.profile).to_lowercase(),
                ),
                PathBuf::from(".hdd/root/linuiz/"),
            )?;
        }
    }

    /* disassemble kernel */
    if options.disassemble {
        let output = cmd!(shell, "objdump -D .hdd/root/linuiz/kernel.elf").output()?;
        shell.write_file(PathBuf::from(".debug/disassembly"), output.stdout)?;
    }

    if options.readelf {
        let output = cmd!(shell, "readelf -hlS .hdd/root/linuiz/kernel.elf").output()?;
        shell.write_file(PathBuf::from(".debug/readelf"), output.stdout)?;
    }

    Ok(())
}
