## Linuiz OS Standardized ABI
#### Note: this document is subject to change, so long as the OS version number is <1.0.

### ELF format
The OS expects to find a specific section within each binary, called `.lz_params`. The layout of this section is as follows:

| Offset |                      |                         |                   |                |
|--------|----------------------|-------------------------|-------------------|----------------|
| 0x0    | Requested Stack Size | is implemented (1 bool) | padding (7 bytes) | value (8 bytes |

*note: the layout expects the same endianness as the ELF specifies in its header*

### Syscall Calling Convention
To perform a system call, software raises a `30h` interrupt. On x86_64, parameters are passed in accordance with the System V ABI specification, which can be found [here](https://www.uclibc.org/docs/psABI-x86_64.pdf), with `rdi` containing the system call vector. For RISC-V-based processors, parameters are passed in the first 6 argument registers (`a0` to `a5`), with `a0` being the system call vector.

