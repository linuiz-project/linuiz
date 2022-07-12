# Linuiz OS

**Linuiz OS** is a semi-modular operating system written in the Rust programming language. Its intent is to find the best of both worlds: combine the excellent IPC performance of modular kernel configurations, with the extremely low memory overhead of monolithic kernels.

As it stands, there is very little user-facing functionality. The bootloader (Limine + Limine boot protocol) can be configured via the `resources/limine.cfg` file. OVMF is used for the UEFI firmware.


## Running the OS
In order to build and run Linuiz OS, you'll require a few things:
- QEMU installed and in your PATH
- GNUMake installed and in your PATH
- Rust (rustup/cargo) installed and configured to use latest nightly.

Some additional requirements:
- Current, xAPIC support is nonexitent. It's been removed in favor of x2APIC. Specifically, this means true hardware virtualization must be used, usually via KVM. QEMU's TCG emulator will not work.

Once you've met those requirements——open a terminal, navigate to the root working direcotry of the project, and type  `make run`. GNUMake should do the rest!