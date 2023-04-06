apt-get update;
apt-get install -y \
    curl \
    git \
    qemu \
    qemu-utils
    
curl https://sh.rustup.rs -sSf | sh -s -- --profile minimal --default-toolchain nightly --component rustfmt,clippy -y
source "$HOME/.cargo/env"

rustup toolchain add x86-64-unknown-none
rustup toolchain add riscv64gc-unknown-none-elf
rustup --version
cargo --version
rustc --version