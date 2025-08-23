use anyhow::Result;
use std::path::Path;
use xshell::Shell;

/// Possible target platforms to compile for.
#[allow(non_camel_case_types)]
#[derive(Debug, ValueEnum, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub enum Target {
    x86_64,
    riscv64,
    aarch64,
}

impl Target {
    pub const fn as_triple(&self) -> &'static str {
        match self {
            Target::x86_64 => "x86_64-unknown-none",
            Target::riscv64 => "riscv64gc-unknown-none",
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

    /// Whether to produce a disassembly of the kernel.
    #[arg(short = 'y', long)]
    disassemble: bool,
}

pub fn build(sh: &Shell, temp_dir: impl AsRef<Path>, options: Options) -> Result<()> {
    let _cargo_log = sh.push_env(
        "CARGO_LOG",
        if options.fingerprint {
            "cargo::core::compiler::fingerprint=info"
        } else {
            ""
        },
    );

    let root_dir = sh.current_dir();

    build_kernel(sh, temp_dir.as_ref(), &options)?;

    if !sh.path_exists("run/system/linuiz") {
        sh.create_dir("run/system/linuiz")?;
    }

    let kernel_path = root_dir.join("run/system/linuiz/kernel");
    // Copy the kernel binary to the virtual HDD.
    sh.copy_file(temp_dir.as_ref().join("kernel"), kernel_path.as_path())?;

    if options.disassemble {
        let disassembly_output = cmd!(
            sh,
            "objdump --disassemble-all --demangle=rust -M intel {kernel_path}"
        )
        .output()?;
        sh.write_file(
            root_dir.join(".debug/kernel.dump"),
            disassembly_output.stdout.as_slice(),
        )?;
    }

    Ok(())
}

fn build_kernel(sh: &Shell, temp_dir: impl AsRef<Path>, options: &Options) -> Result<()> {
    let mut build_cmd = cmd!(sh, "cargo build")
        .arg("--future-incompat-report")
        .arg("--artifact-dir")
        .arg(temp_dir.as_ref().as_os_str())
        .args(["--target", options.target.as_triple()])
        .args(["-Z", "unstable-options"])
        .args(["-Z", "build-std=core,compiler_builtins,alloc"])
        .args(["-Z", "build-std-features=compiler-builtins-mem"]);

    if options.release {
        build_cmd = build_cmd.arg("--release");
    }

    if options.verbose {
        build_cmd = build_cmd.arg("-vv")
    }

    // Set RUSTFLAGS to enable custom getrandom backend.
    let _rustflags = sh.push_env("RUSTFLAGS", "--cfg=getrandom_backend=\"custom\"");

    build_cmd.run()?;

    Ok(())
}
