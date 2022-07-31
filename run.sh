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
    -bios resources/OVMF.fd \
    -drive format=raw,file=fat:rw:./.hdd/root/ \
    -drive format=raw,file=./.hdd/disk.img,id=disk,if=none \
    -device virtio-blk-pci,drive=disk,serial=deadbeef \
    -D .debug/qemu_debug.log \
    -d int,guest_errors

