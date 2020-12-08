qemu-system-x86_64^
    -bios ./ovmf/OVMF-pure-efi.fd^
    -drive format=raw,file=fat:rw:./image/^
    -serial stdio