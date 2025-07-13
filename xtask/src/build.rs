use anyhow::Result;
use std::{env::set_var, fs::File, path::Path};
use xshell::Shell;

/// Possible target platforms to compile for.
#[allow(non_camel_case_types)]
#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub enum Target {
    x86_64,
    riscv64gc,
    aarch64,
}

impl Target {
    pub const fn as_triple(&self) -> &'static str {
        match self {
            Target::x86_64 => "x86_64-unknown-none",
            Target::riscv64gc => unimplemented!(),
            Target::aarch64 => unimplemented!(),
        }
    }
}

#[derive(Parser)]
pub struct Options {
    /// Verbose build output. Equivalent to `cargo build -vv`.
    #[arg(short, long)]
    verbose: bool,

    /// Whether to print the kernel's build fingerprint.
    /// This can be useful for debugging constant rebuilds.
    #[arg(long)]
    fingerprint: bool,

    /// Target platform to compile for.
    #[arg(short, long)]
    target: Target,

    /// Whether to build in release mode (with all optimizations).
    #[arg(long)]
    release: bool,

    #[arg(long)]
    drivers: Vec<String>,
}

pub fn build(sh: &Shell, temp_dir: impl AsRef<Path>, options: Options) -> Result<()> {
    cmd!(sh, "cargo fmt --check").run()?;
    cmd!(sh, "cargo sort --workspace --grouped --check").run()?;

    // Safety: Single-threaded.
    unsafe {
        set_var("LINUIZ_OUT_DIR", temp_dir.as_ref().as_os_str());
    }

    if options.fingerprint {
        // Safety: Single-threaded.
        unsafe {
            set_var("CARGO_LOG", "cargo::core::compiler::fingerprint=info");
        }
    }

    let root_dir = sh.current_dir();

    let mut build_cmd = cmd!(sh, "cargo build")
        .arg("--target")
        .arg(options.target.as_triple())
        .arg("--artifact-dir")
        .arg(temp_dir.as_ref().as_os_str())
        .arg("-Z")
        .arg("unstable-options");

    if options.release {
        build_cmd = build_cmd.arg("--release");
    } else {
        // Only provide future-compatibiltiy notifications for development builds.
        build_cmd = build_cmd.arg("--future-incompat-report")
    }

    if options.verbose {
        build_cmd = build_cmd.arg("-vv")
    }

    build_cmd.run()?;

    if !sh.path_exists("run/system/linuiz") {
        sh.create_dir("run/system/linuiz")?;
    }

    // Copy the kernel binary to the virtual HDD.
    sh.copy_file(
        temp_dir.as_ref().join("kernel"),
        root_dir.join("run/system/linuiz/kernel"),
    )?;

    // compress userspace drivers and write to archive file
    let mut archive_builder = tar::Builder::new(
        File::create(root_dir.join("run/system/linuiz/drivers"))
            .expect("failed to create or open the driver package file"),
    );

    sh.read_dir(temp_dir.as_ref())?
        .into_iter()
        .filter(|p| {
            p.file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .filter(|driver_name| options.drivers.iter().any(|s| s.eq(driver_name)))
                .is_some()
        })
        .try_for_each(|path| {
            println!("Packaging driver: {:?}", path.file_name().unwrap());

            let rel_path = path.strip_prefix(temp_dir.as_ref()).unwrap();
            archive_builder.append_file(rel_path, &mut File::open(&path)?)
        })?;

    archive_builder.finish()?;

    Ok(())
}
