mod build;
mod run;
mod target;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;
use xshell::{cmd, Shell};

static WORKSPACE_DIRS: [&str; 3] = ["src/kernel", "src/shared", "src/userspace"];

static UEFI_FIRMWARE_IMAGE_URL: &str = "https://github.com/rust-osdev/ovmf-prebuilt/releases/download/edk2-stable202211-r1/edk2-stable202211-r1-bin.tar.xz";
static LIMINE_UEFI_IMAGE_URL: &str =
    "https://raw.githubusercontent.com/limine-bootloader/limine/v4.x-branch-binary/BOOTX64.EFI";
static LIMINE_DEFAULT_CFG: &str = r#"
TIMEOUT=3
SERIAL=yes

:Pyre (limine)
COMMENT=Load Pyre OS using the Limine boot protocol.
PROTOCOL=limine
RESOLUTION=800x600x16
KERNEL_PATH=boot:///pyre/kernel
MODULE_PATH=boot:///pyre/drivers
KASLR=yes
"#;

#[derive(Parser)]
struct Fmt {
    args: Vec<String>,
}

#[derive(Parser)]
#[command(rename_all = "snake_case")]
enum Arguments {
    Clean,
    Update,
    Check,
    Clippy,
    Fmt(Fmt),

    #[command(subcommand)]
    Target(target::Target),

    Build(build::Options),
    Run(run::Options),
}

fn main() -> Result<()> {
    let sh = Shell::new()?;

    if !sh.path_exists(".debug/") {
        sh.create_dir(".debug/")?;
    }

    // Ensure the binaries have their `.cargo/config.toml`s.
    if !sh.path_exists("src/kernel/.cargo/config.toml") || !sh.path_exists("src/userspace/.cargo/config.toml") {
        target::update_target(&sh, target::Target::x86_64)?;
    }

    // Ensure we use the 'sparse' cargo repository protocol
    sh.set_var("CARGO_REGISTRIES_CRATES_IO_PROTOCOL", "sparse");

    // Validate all of the relevant files
    create_path_if_not_exists(&sh, "build/root/EFI/BOOT/")?;
    create_path_if_not_exists(&sh, "build/root/pyre/")?;
    // Ensure dev disk image exists.
    if !sh.path_exists("build/disk0.img") {
        cmd!(sh, "qemu-img create -f raw build/disk0.img 256M").run()?;
    }

    // Ensure a valid bootloader configuration exists.
    if !sh.path_exists("build/root/EFI/BOOT/limine.cfg") {
        sh.write_file("build/root/EFI/BOOT/limine.cfg", LIMINE_DEFAULT_CFG)?;
    }

    // Download UEFI boot image.
    if !sh.path_exists("build/root/EFI/BOOT/BOOTX64.EFI") {
        println!("Downloading limine UEFI boot image.");
        cmd!(sh, "curl -o build/root/EFI/BOOT/BOOTX64.EFI {LIMINE_UEFI_IMAGE_URL}").run()?;
    }

    if !sh.path_exists("build/OVMF_CODE.fd") || !sh.path_exists("build/OVMF_VARS.fd") {
        println!("Downloading UEFI firmware image.");

        let tmp_dir = sh.create_temp_dir()?;
        let tar_path = tmp_dir.path().join("ovmf_prebuild.tar.xz");
        cmd!(sh, "curl -o {tar_path} {UEFI_FIRMWARE_IMAGE_URL}").run()?;

        let mut archive =
            tar::Archive::new(std::fs::File::open(tar_path).with_context(|| "failed opening OVMF image TAR")?);

        archive.entries().with_context(|| "archive has no entries")?.flatten().try_fold(
            (false, false),
            |(has_code, has_vars), mut entry| {
                let path = entry.path().map(|path| path.to_string_lossy().into_owned());
                match path.as_deref() {
                    Ok("x64/code.fd") => {
                        println!("Found entry for EFI code: {:?}", entry.path());

                        entry
                            .unpack_in("build/OVMF_CODE.fd")
                            .with_context(|| "could not unpack OVMF_CODE.fd")
                            .map(|_| (true, has_vars))
                    }

                    Ok("x64/vars.fd") => {
                        println!("Found entry for EFI vars: {:?}", entry.path());

                        entry
                            .unpack_in("build/OVMF_VARS.fd")
                            .with_context(|| "could not unpack OVMF_VARS.fd")
                            .map(|_| (has_code, true))
                    }

                    _ => {
                        println!("Found entry, skipping: {:?}", entry.path());

                        Ok((has_code, has_vars))
                    }
                }
            },
        )?;
    }

    match Arguments::parse() {
        Arguments::Clean => {
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo clean").run().with_context(|| "`cargo clean` failed"))
        }
        Arguments::Check => {
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo check --bins").run().with_context(|| "`cargo check` failed"))
        }
        Arguments::Update => {
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo update").run().with_context(|| "`cargo update` failed"))
        }
        Arguments::Clippy => {
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo clippy").run().with_context(|| "`cargo clippy` failed"))
        }
        Arguments::Fmt(fmt) => {
            let args = &fmt.args;
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo fmt {args...}").run().with_context(|| "`cargo fmt` failed"))
        }

        Arguments::Target(target) => target::update_target(&sh, target).with_context(|| "failed to"),
        Arguments::Build(build_options) => build::build(&sh, build_options),
        Arguments::Run(run_options) => run::run(&sh, run_options),
    }
}

fn in_workspace_with(shell: &Shell, with_fn: impl Fn(&Shell) -> Result<()>) -> Result<()> {
    for dir in WORKSPACE_DIRS {
        let _dir = shell.push_dir(dir);
        with_fn(shell)?
    }

    Ok(())
}

fn create_path_if_not_exists<P: AsRef<Path>>(sh: &Shell, path: P) -> Result<()> {
    if !sh.path_exists(path.as_ref()) {
        sh.create_dir(path.as_ref())?;
    }

    Ok(())
}
