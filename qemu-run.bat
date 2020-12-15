qemu-system-x86_64^
    -m 512M^
    -vga std^
    -bios ./ovmf/OVMF-pure-efi.fd^
    -drive format=raw,file=fat:rw:./image/^
    -serial stdio