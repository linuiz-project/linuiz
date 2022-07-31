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
}

static REQUIRED_ROOT_DIRS: [&str; 2] = [".hdd/", ".debug/"];

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
            // update the submodule to ensure latest version
            cmd!(shell, "git submodule update").run()?;
            // copy the resultant EFI binary
            shell.copy_file(PathBuf::from("submodules/limine/BOOTX64.EFI"), PathBuf::from(".hdd/root/EFI/BOOT/"))?;
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

    Ok(())
}
