# Pyre OS

**Pyre OS** is a semi-modular operating system written in the Rust programming language. Its intent is to find the best of both worlds: combine the excellent IPC performance of modular kernel configurations, with the extremely low memory overhead of monolithic kernels.

As it stands, there is very little user-facing functionality. The bootloader (Limine + Limine boot protocol) can be configured via the `resources/limine.cfg` file. OVMF is used for the UEFI firmware.

Driver ELF loading (and general userspace process loading) is coming soon.


## Running the OS
In order to build and run Pyre OS, you'll require a few things:
- QEMU installed and in your PATH
- LLVM installed and in your PATH
- Rust (rustup/cargo) installed and configured to use latest nightly.

Some additional requirements:
- To utilize the `-d` disassembly and `-r` readelf flags, you'll need both `llvm-objdump` and `readelf` binaries in your PATH.

Once you've met those requirements——open a terminal, navigate to the root working directory of the project (`pyre/`), type the following commands:
- `cargo xtask build --target x64`
- `cargo xtask run --cpu qemu64 --smp 4 --log`