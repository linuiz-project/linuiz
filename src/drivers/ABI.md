## Linuiz OS Standardized ABI Specification
#### Note: this document is subject to change, so long as the OS version number is <1.0.

### Process Stacks
The kernel places new process / task stacks at `0x400000800000`.

### Syscall Calling Convention
To perform a system call, software raises a `30h` interrupt. On x86_64, parameters are passed in accordance with the System V ABI specification, which can be found [here](https://www.uclibc.org/docs/psABI-x86_64.pdf), with `rdi` containing the system call vector. For RISC-V-based processors, parameters are passed in the first 6 argument registers (`a0` to `a5`), with `a0` being the system call vector.

