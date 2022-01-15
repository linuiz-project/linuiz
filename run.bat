qemu-system-x86_64^
    -m 256M^
    -serial stdio^
    -machine q35^
    -cpu qemu64^
    -smp 8^
    -bios ./ovmf.fd^
    -drive format=raw,file=fat:rw:./.hdd/image/^
    -drive if=none,format=raw,id=nvm,file=./.hdd/nvme.img^
    -device nvme,drive=nvm,serial=deadbeef^
    -net none^
    -no-reboot^
    -D ./.debug/qemu_debug.log^
    -d guest_errors,int

