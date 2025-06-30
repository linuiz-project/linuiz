use std::{
    env::set_var,
    fs::File,
    hash::{DefaultHasher, Hash, Hasher},
    io::Error,
    path::Path,
};

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

/// Possible segment alignments for the kernel.
#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentAlign {
    /// Single-page alignment.
    Small,

    /// Mega-page alignment.
    Fast,
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

    /// Page alignment of the kernel executable segments.
    #[arg(long, default_value = "fast")]
    kalign: SegmentAlign,

    #[arg(long)]
    drivers: Vec<String>,
}

pub fn build<P: AsRef<Path>>(
    sh: &xshell::Shell,
    temp_dir: P,
    options: Options,
) -> anyhow::Result<()> {
    if options.fingerprint {
        // Safety: Usage is single-threaded.
        unsafe {
            set_var("CARGO_LOG", "cargo::core::compiler::fingerprint=info");
        }
    }

    // Save a fingerprint for options Cargo doesn't track (is used by `cargo:rerun-if-changed`).
    let mut hasher = DefaultHasher::new();
    options.kalign.hash(&mut hasher);
    if let Err(error) = sh.write_file("target/.xtraprint", hasher.finish().to_le_bytes()) {
        println!("Couldn't write kernel extra fingerprint: {error:?}");
    }

    // Safety: Usage is single-threaded.
    unsafe {
        set_var(
            "KERNEL_SEGMENT_ALIGN",
            match options.kalign {
                SegmentAlign::Small if options.target == Target::x86_64 => "0x1000",
                SegmentAlign::Fast if options.target == Target::x86_64 => "0x200000",

                _ => unimplemented!(),
            },
        );
    }

    let root_dir = sh.current_dir();

    let mut build_cmd = cmd!(sh, "cargo build")
        .args(["--target", options.target.as_triple()])
        .args(["--artifact-dir", temp_dir.as_ref().to_str().unwrap()])
        .args(["-Z", "unstable-options"]);

    if options.release {
        cmd!(sh, "cargo fmt --check").run()?;

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

    build_drivers_archive(
        temp_dir.as_ref(),
        root_dir.join("run/system/linuiz/drivers"),
        sh.read_dir(temp_dir.as_ref())?.into_iter(),
        &options.drivers,
    )
    .expect("error attempting to package drivers");

    Ok(())
}

fn build_drivers_archive<P1: AsRef<Path>, P2: AsRef<Path>>(
    drivers_path: P1,
    archive_path: P2,
    files: impl Iterator<Item = std::path::PathBuf>,
    include_drivers: &[String],
) -> Result<(), Error> {
    let drivers_path = drivers_path.as_ref();

    // compress userspace drivers and write to archive file
    let mut archive_builder = tar::Builder::new(
        File::create(archive_path).expect("failed to create or open the driver package file"),
    );

    files
        .filter(|p| {
            p.file_name()
                .map(std::ffi::OsStr::to_string_lossy)
                .filter(|driver_name| include_drivers.iter().any(|s| s.eq(driver_name)))
                .is_some()
        })
        .try_for_each(|path| {
            println!("Packaging driver: {:?}", path.file_name().unwrap());

            let rel_path = path.strip_prefix(drivers_path).unwrap();
            archive_builder.append_file(rel_path, &mut File::open(&path)?)
        })?;

    archive_builder.finish()
}
