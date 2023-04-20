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

    /// Whether to print the kernel's build fingerprint.
    /// This can be useful for debugging constant rebuilds.
    #[arg(long)]
    fingerprint: bool,

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

    let _cargo_log = {
        let mut cargo_log = Vec::new();

        if options.fingerprint {
            cargo_log.push("cargo::core::compiler::fingerprint=info");
        }

        sh.push_env("CARGO_LOG", cargo_log.join(" "))
    };

    let root_dir = sh.current_dir();

    let tmp_dir = sh.create_temp_dir()?;
    let tmp_dir_path = tmp_dir.path();
    let tmp_path_dir_str = tmp_dir_path.to_string_lossy();

    let cargo_args = {
        let mut args = vec![
            "--out-dir",
            &tmp_path_dir_str,
            if options.release {
                "--release"
            } else {
                // Only provide future-compatibiltiy notifications for development builds.
                "--future-incompat-report"
            },
        ];

        if options.verbose {
            args.push("-vv");
        }

        args
    };

    /* compile kernel */
    {
        let _dir = sh.push_dir("src/kernel/");
        // Configure rustc via the `RUSTFLAGS` environment variable for the kernel build.
        let _rustflags = sh.push_env("RUSTFLAGS", vec!["-C code-model=kernel", "-C embed-bitcode=yes"].join(" "));

        cmd!(sh, "cargo fmt").run()?;
        let local_args = &cargo_args;
        cmd!(sh, "cargo build --bins -Z unstable-options {local_args...}").run()?;

        // Copy the output kernel binary to the virtual HDD.
        sh.copy_file(tmp_dir_path.join("kernel"), root_dir.join(".hdd/root/pyre/"))?;
    }

    /* compile userspace */
    {
        let _dir = sh.push_dir("src/userspace/");

        cmd!(sh, "cargo fmt").run()?;
        let local_args = &cargo_args;
        cmd!(sh, "cargo build --bins -Z unstable-options {local_args...}").run()?;
    }

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
