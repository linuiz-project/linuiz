# Gsai OS

## About
**Gsai OS** is a semi-modular operating system written in the Rust programming language. Its intent is to find the best of both worlds: combine the excellent performance of modular kernel configurations, with the extremely low memory overhead of monolithic kernels.

</br>

## Running the OS
In order to build and run Gsai OS, you'll require a few things:
- QEMU installed and in your PATH
- NASM installed and in your PATH
- GNUMake installed and in your PATH
- Rust (rustup/cargo) installed and configured to use latest nightly.

Once you've met those requirements, open a terminal, navigate to the root working direcotry of the project, and type  `make run` —GNUMake should do the rest!