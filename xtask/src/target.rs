use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use xshell::Shell;

#[derive(Debug, Clone, Copy, clap::Subcommand)]
#[allow(non_camel_case_types)]
pub enum Target {
    x86_64,
    riscv64gc,
}

impl core::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::x86_64 => "x86_64-unknown-none",
            Self::riscv64gc => "riscv64gc-unknown-none-elf",
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ConfigOptions {
    unstable_options: UnstableOptions,
    build_options: BuildOptions,
}

impl ConfigOptions {
    pub fn new(target: Target, rustflags: Option<Vec<String>>) -> Self {
        Self {
            unstable_options: UnstableOptions {
                build_std: vec!["core".into(), "compiler_builtins".into(), "alloc".into()],
                build_std_features: vec!["compiler-builtins-mem".into()],
            },
            build_options: BuildOptions { target: target.to_string(), rustflags },
        }
    }
}

#[derive(Serialize, Deserialize)]
struct UnstableOptions {
    #[serde(rename = "build-std")]
    build_std: Vec<String>,

    #[serde(rename = "build-std-features")]
    build_std_features: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct BuildOptions {
    target: String,
    rustflags: Option<Vec<String>>,
}

pub fn update_target(sh: &Shell, target: Target) -> Result<()> {
    // kernel target update
    update_target_impl(
        sh,
        "src/kernel/.cargo/config.toml",
        target,
        Some(vec![
            "-C".into(),
            "code-model=kernel".into(),
            "-C".into(),
            "embed-bitcode=yes".into(),
            "--cfg".into(),
            "getrandom_backend=\"custom\"".into(),
        ]),
    )?;

    // userspace target update
    update_target_impl(sh, "src/userspace/.cargo/config.toml", target, None)?;

    Ok(())
}

fn update_target_impl(
    sh: &Shell,
    path: impl AsRef<std::path::Path>,
    target: Target,
    rustflags: Option<Vec<String>>,
) -> Result<()> {
    let config = ConfigOptions::new(target, rustflags);
    let config_toml = toml::to_string_pretty(&config).with_context(|| "failed prettifying config TOML")?;
    sh.write_file(path.as_ref(), config_toml).with_context(|| "failed writing prettified TOML")?;

    Ok(())
}
