qemu-system-x86_64^
    -m 256M^
    -serial stdio^
    -machine q35^
    -cpu qemu64,+x2apic^
    -drive format=raw,file=fat:rw:./image/^
    -drive if=pflash,format=raw,unit=0,file=./ovmf/OVMF_CODE-pure-efi.fd,readonly=on^
    -drive if=pflash,format=raw,unit=1,file=./ovmf/OVMF_VARS-pure-efi.fd,readonly=on^
    -net none^
