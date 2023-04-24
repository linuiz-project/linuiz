use serde::{Deserialize, Serialize};
use xshell::{Result, Shell};

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
    unstable: UnstableOptions,
    build: BuildOptions,
}

#[derive(Serialize, Deserialize)]
struct UnstableOptions {
    #[serde(rename = "build-std")]
    build_std: Vec<String>,

    #[serde(rename = "build-std-features")]
    build_std_features: Vec<String>,
}

impl Default for UnstableOptions {
    fn default() -> Self {
        Self {
            build_std: vec!["core".into(), "compiler_builtins".into(), "alloc".into()],
            build_std_features: vec!["compiler-builtins-mem".into()],
        }
    }
}

#[derive(Serialize, Deserialize)]
struct BuildOptions {
    target: String,
    rustflags: Option<Vec<String>>,
}

impl ConfigOptions {
    fn kernel_default(target: Target) -> Self {
        Self {
            unstable: Default::default(),
            build: BuildOptions {
                target: target.to_string(),
                rustflags: Some(vec!["-C".into(), "code-model=kernel".into(), "-C".into(), "embed-bitcode=yes".into()]),
            },
        }
    }

    fn userspace_default(target: Target) -> Self {
        Self { unstable: Default::default(), build: BuildOptions { target: target.to_string(), rustflags: None } }
    }
}

pub fn update_target(sh: &Shell, target: Target) -> Result<()> {
    update_target_path(sh, target, "src/kernel/.cargo/config.toml", || ConfigOptions::kernel_default(target))?;
    update_target_path(sh, target, "src/userspace/.cargo/config.toml", || ConfigOptions::userspace_default(target))?;

    Ok(())
}

fn update_target_path<P: AsRef<std::path::Path>>(
    sh: &Shell,
    target: Target,
    path: P,
    default: impl FnOnce() -> ConfigOptions,
) -> Result<()> {
    let config = sh
        .read_file(path.as_ref())
        .map_err(|_| ())
        .and_then(|config_file| {
            toml::from_str::<ConfigOptions>(&config_file).map_err(|_| ()).map(|mut config| {
                config.build.target = target.to_string();
                config
            })
        })
        .unwrap_or_else(|_| default());

    sh.write_file(path.as_ref(), toml::to_string_pretty(&config).unwrap())
}
