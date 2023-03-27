use std::{fs::File, io::Error, path::Path};
use xshell::{cmd, Result, Shell};

#[derive(clap::Parser)]
#[allow(non_snake_case)]
pub struct Options {
    /// Whether the current build is a release build.
    #[arg(long)]
    release: bool,

    /// Verbose build output. Equivalent to `cargo build -vv`.
    #[arg(short, long)]
    verbose: bool,

    #[arg(long, default_value = "test_driver")]
    drivers: Vec<String>,
}

static REQUIRED_ROOT_DIRS: [&str; 3] = [
    ".hdd/",               // disk0.img
    ".hdd/root/EFI/BOOT/", // BOOTX64.EFI
    ".hdd/root/pyre/",     // kernel, drivers
];

pub fn build(sh: &Shell, options: Options) -> Result<()> {
    // Ensure root directories exist
    for root_dir in REQUIRED_ROOT_DIRS {
        if !sh.path_exists(root_dir) {
            sh.create_dir(root_dir)?;
        }
    }

    // Ensure dev disk image exists.
    if !sh.path_exists(".hdd/disk0.img") {
        cmd!(sh, "qemu-img create -f raw .hdd/disk0.img 256M").run()?;
    }

    // copy configuration to EFI image
    sh.copy_file("resources/limine.cfg", ".hdd/root/EFI/BOOT/")?;
    // copy the EFI binary image
    sh.copy_file("resources/BOOTX64.EFI", ".hdd/root/EFI/BOOT/")?;

    // Configure rustc via the `RUSTFLAGS` environment variable.
    let _rustflags = {
        let mut rustflags = Vec::new();
        rustflags.push("-C relocation_model=static");
        rustflags.push("-C code-model=kernel");
        rustflags.push("-C embed-bitcode=yes");

        // For debug builds, we always enable frame pointers for pretty stack tracing.
        if !options.release {
            rustflags.push("-C force-frame-pointers");
            rustflags.push("-C symbol-mangling-version=v0");
        }

        sh.push_env("RUSTFLAGS", rustflags.join(" "))
    };

    let root_dir = sh.current_dir();
    let _dir = sh.push_dir("src/");
    let tmp_dir = sh.create_temp_dir()?;
    let tmp_dir_path = tmp_dir.path();
    let tmp_path_dir_str = tmp_dir_path.to_string_lossy();

    let cargo_args = {
        let mut args = vec!["--out-dir", &tmp_path_dir_str];

        args.push({
            if options.release {
                "--release"
            } else {
                // Only provide future-compatibiltiy notifications for development builds.
                "--future-incompat-report"
            }
        });

        if options.verbose {
            args.push("-vv");
        }

        args
    };

    cmd!(sh, "cargo build --bins -Z unstable-options {cargo_args...}").run()?;
    // Copy the output kernel binary to the virtual HDD.
    sh.copy_file(tmp_dir_path.join("kernel"), root_dir.join(".hdd/root/pyre/"))?;

    build_drivers_archive(
        tmp_dir_path,
        &root_dir.join(".hdd/root/pyre/drivers"),
        sh.read_dir(tmp_dir_path)?.into_iter(),
        &options.drivers,
    )
    .expect("error attempting to package drivers");

    Ok(())
}

fn build_drivers_archive<P: AsRef<Path>>(
    drivers_path: P,
    archive_path: P,
    files: impl Iterator<Item = std::path::PathBuf>,
    include_drivers: &[String],
) -> Result<(), Error> {
    let drivers_path = drivers_path.as_ref();

    // compress userspace drivers and write to archive file
    let mut archive_builder =
        tar::Builder::new(File::create(archive_path).expect("failed to create or open the driver package file"));

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
            archive_builder.append_file(&rel_path, &mut File::open(&path)?)
        })?;

    archive_builder.finish()
}
