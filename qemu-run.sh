# exits script on command error
set -e

# compile and link deps
make $1

# run the bootloader image
qemu-system-x86_64 \
    -nodefaults \
    -nographic \
    -bios ./ovmf/OVMF-pure-efi.fd \
    -drive format=raw,file=fat:rw:./image/ \
    -serial stdio
