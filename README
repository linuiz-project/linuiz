# **Pyre OS**

*Pyre* is a semi-modular operating system written in the Rust programming language. Its intent is to find the best of both worlds: combine the excellent IPC performance of modular kernel configurations, with the extremely low memory overhead of monolithic kernels.

As it stands, there is very little user-facing functionality. The bootloader (Limine + Limine boot protocol) can be configured via the `resources/limine.cfg` file. At time of writing, there are a few configurables for the kernel:

- `--nosmp` will park the additional CPU cores
- `--symbolinfo` will retain symbol info, even in release builds (use for kernel stack traces)
- `--lomem` will do several different things in an attempt to lower memory usage

Currently, drivers are packaged and loaded as a tarball, but will yet be loaded into userspace and ran (this is WIP).


## **Building / Running the OS**
---

**Prerequisites:**
 - cargo + rustup installed and in your PATH.

### Building
To build, simply run `cargo xtask build` within the root project directory. The resultant binaries will be output to `.hdd/root/pyre/`.

### Running
`cargo xtask run` will both build and run the OS (so build flags can be passed to this command as well).