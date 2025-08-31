use anyhow::Result;
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

    /// Optimizes the build for use in GitHub Actions. This will for example
    /// ignore flags like `--disassemble`.
    #[arg(long)]
    github_actions: bool,
}

pub fn build(sh: &Shell, options: Options) -> Result<()> {
    let _cargo_log = sh.push_env(
        "CARGO_LOG",
        if options.fingerprint {
            "cargo::core::compiler::fingerprint=info"
        } else {
            ""
        },
    );

    build_kernel(sh, &options)?;

    if options.github_actions {
        return Ok(());
    }

    let root_dir = sh.current_dir();
    let kernel_src_path = root_dir.join(format!(
        "target/{}/{}/kernel",
        options.target.as_triple(),
        if options.release { "release" } else { "debug" }
    ));
    let kernel_dst_dir = root_dir.join("run/system/linuiz/");

    let kernel_src_path = kernel_src_path.as_path();
    let kernel_dst_dir = kernel_dst_dir.as_path();

    if !sh.path_exists(kernel_dst_dir) {
        sh.create_dir(kernel_dst_dir)?;
    }

    // Copy the kernel binary to the virtual HDD.
    sh.copy_file(kernel_src_path, kernel_dst_dir)?;

    if options.disassemble {
        let disassembly_output = cmd!(
            sh,
            "objdump --disassemble-all --demangle=rust -M intel {kernel_dst_dir}/kernel"
        )
        .output()?;
        sh.write_file(
            root_dir.join(".debug/kernel.dump"),
            disassembly_output.stdout.as_slice(),
        )?;
    }

    Ok(())
}

fn build_kernel(sh: &Shell, options: &Options) -> Result<()> {
    let target_triple = options.target.as_triple();

    let mut build_cmd = cmd!(sh, "cargo build")
        .arg("--future-incompat-report")
        .args(["--target", target_triple])
        .args(["-Z", "unstable-options"]);

    if options.release {
        build_cmd = build_cmd.arg("--release");
    }

    if options.verbose {
        build_cmd = build_cmd.arg("-vv")
    }

    let _rustflags = sh.push_env(
        "RUSTFLAGS",
        format!(
            "--cfg=getrandom_backend=\"custom\" \
            -C link-arg=-Tbuild/{target_triple}.lds \
            -C link-arg=build/{target_triple}.a \
            -C link-arg=-zmax-page-size=0x200000",
        ),
    );

    build_cmd.run()?;

    Ok(())
}
