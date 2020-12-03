qemu-system-x86_64^
    -nodefaults^
    -nographic^
    -bios ./ovmf/OVMF-pure-efi.fd^
    -drive format=raw,file=fat:rw:../image/^
    -serial stdio