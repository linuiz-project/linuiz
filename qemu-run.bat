qemu-system-x86_64^
    -m 256M^
    -serial stdio^
    -debugcon stdio^
    -cpu qemu64,+x2apic^
    -bios ./ovmf/OVMF-pure-efi.fd^
    -drive format=raw,file=fat:rw:./image/
