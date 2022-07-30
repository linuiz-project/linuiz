use clap::{clap_derive::ArgEnum, Parser};
use std::path::PathBuf;
use xshell::{cmd, Shell};

#[derive(Clone, Copy, ArgEnum)]
pub enum Profile {
    Release,
    Debug,
}

impl core::fmt::Debug for Profile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Profile::Release => "release",
            Profile::Debug => "dev",
        })
    }
}

#[derive(Parser)]
pub struct Options {
    /// Whether the current build is debug mode or not.
    #[clap(arg_enum, long)]
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

        if !shell.path_exists(".hdd/disk.img") {
            cmd!(shell, "qemu-img create -f raw .hdd/disk.img 256M").run()?;
        }
    }

    /* build */
    {
        {
            // libkernel
            let _dir = shell.push_dir("src/libkernel/");
            cmd!(shell, "cargo fmt").run()?;
        }

        {
            //kernel
            let _dir = shell.push_dir("src/kernel/");
            let profile_str = format!("{:?}", options.profile);

            cmd!(shell, "cargo fmt").run()?;
            cmd!(shell, "cargo build --profile {profile_str} -Z unstable-options").run()?;
        }
    }

    /* disassemble kernel */
    if options.disassemble {
        let output = cmd!(shell, "objdump -D .hdd/root/linuiz/kernel.elf").output()?;
        shell.write_file(PathBuf::from(".debug/disassembly"), output.stdout)?;
    }

    Ok(())
}
