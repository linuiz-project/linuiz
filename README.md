## What is it?

*Pyre* is a semi-modular multi-archutecture operating system written in the Rust programming language.

## Why is it?

Pyre is an experiment that seeks to abridge the differences between many separate programming and kernel design paradigms. 
Additionally, Pyre aims to seamlessly integrate a windowing into the command-line experience—creating a hybrid that allows traditionally technical environments to be more accessible.

<br />


# **Testing It Out**

## Building
The build process is mostly automated via the `cargo xtask` pattern, although the following **prerequisites** must be met:
  - `cargo` and `rustup` installed and in your `PATH`.
  - The following packages installed:

    &ensp;`git`, `ovmf`, `gcc`, `qemu`, `qemu-utils`

  - Depending on the architecture you wish to target, you may need one of the following:

    &ensp;`qemu-system-x86`, `qemu-system-arm`, or `qemu-system-misc` *(for risc-v)*

    &ensp;Others can be listed with `apt list | grep qemu-system`

<br />

## Running

To run the OS with its default configuration, simply call: `cargo xtask run`

<!-- TODO list some common options related to the command  -->
