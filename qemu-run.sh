# exits script on command error
set -e

PROFILE=${1:-release}

echo "Compiling with profile '$PROFILE'"

# compile and link deps
make PROFILE=$PROFILE

# run the bootloader image
qemu-system-x86_64 \
    -m 256M \
    -nographic \
    -bios ./ovmf/OVMF-pure-efi.fd \
    -drive format=raw,file=fat:rw:./image/ \
    -D qemu_debug.log \
    -d guest_errors,int