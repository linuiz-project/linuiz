# exits script on command error
set -e

PROFILE=${1:-release}

echo "Compiling with profile '$PROFILE'"

# compile and link deps
make PROFILE=$PROFILE

# run the bootloader image
qemu-system-x86_64 \
    -m 4G \
    -serial stdio \
    -machine q35 \
    -cpu qemu64 \
    -smp 2 \
    -drive format=raw,file=fat:rw:./hdd/image/ \
    -drive if=pflash,format=raw,unit=0,file=./ovmf/OVMF_CODE-pure-efi.fd,readonly=on \
    -drive if=pflash,format=raw,unit=1,file=./ovmf/OVMF_VARS-pure-efi.fd,readonly=on \
    -drive id=disk,if=none,file=./hdd/rootfs.img \