qemu-system-x86_64^
    -m 256M^
    -nographic^
    -cpu qemu64,+x2apic^
    -bios ./ovmf/OVMF-pure-efi.fd^
    -drive format=raw,file=fat:rw:./image/