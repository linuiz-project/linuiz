mod build;
mod run;
mod target;

use clap::Parser;
use std::path::Path;
use xshell::{cmd, Result, Shell};

static WORKSPACE_DIRS: [&str; 3] = ["src/kernel", "src/shared", "src/userspace"];

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
        cmd!(sh, "curl -s -o build/root/EFI/BOOT/BOOTX64.EFI {LIMINE_UEFI_IMAGE_URL}").run()?;
    }

    // Ensure the binaries have their `.cargo/config.toml`s.
    if !sh.path_exists("src/kernel/.cargo/config.toml") || !sh.path_exists("src/userspace/.cargo/config.toml") {
        target::update_target(&sh, target::Target::x86_64)?;
    }

    if !sh.path_exists(".debug/") {
        sh.create_dir(".debug/")?;
    }

    match Arguments::parse() {
        Arguments::Clean => in_workspace_with(&sh, |sh| cmd!(sh, "cargo clean").run()),
        Arguments::Check => in_workspace_with(&sh, |sh| cmd!(sh, "cargo check --bins").run()),
        Arguments::Update => in_workspace_with(&sh, |sh| cmd!(sh, "cargo update").run()),
        Arguments::Clippy => in_workspace_with(&sh, |sh| cmd!(sh, "cargo clippy").run()),
        Arguments::Fmt(fmt) => {
            let args = &fmt.args;
            in_workspace_with(&sh, |sh| cmd!(sh, "cargo fmt {args...}").run())
        }

        Arguments::Target(target) => target::update_target(&sh, target),
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
