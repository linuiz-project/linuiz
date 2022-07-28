#!/bin/bash

qemu-system-x86_64 \
    -no-reboot \
    -machine q35,accel=kvm \
    -cpu host \
    -smp 4 \
    -m 512M \
    -serial mon:stdio \
    -net none \
    -display none \
    -bios ./resources/ovmf.fd \
    -drive format=raw,file=fat:rw:./.hdd/image/ \
    -drive format=raw,file=./.hdd/nvme.img,id=root,if=none \
    -device virtio-blk-pci,drive=root,serial=deadbeef \
    -D .debug/qemu_debug.log \
    -d int,guest_errors

