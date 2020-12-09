# exits script on command error
set -e

PROFILE=${1:-release}

echo "Compiling with profile '$PROFILE'"

# compile and link deps
make PROFILE=$PROFILE

# run the bootloader image
qemu-system-x86_64 \
    -nodefaults \
    -nographic \
    -bios ./ovmf/OVMF-pure-efi.fd \
    -drive format=raw,file=fat:rw:./image/ \
    -serial stdio