#!/bin/bash

qemu-system-x86_64 \
    -no-reboot \
    -machine q35 \
    -cpu max \
    -smp 2 \
    -m 8G \
    -serial mon:stdio \
    -net none \
    -bios ./resources/ovmf.fd \
    -drive format=raw,file=fat:rw:./.hdd/image/ \
    -drive format=raw,file=./.hdd/nvme.img,id=nvm,if=none \
    -device nvme,drive=nvm,serial=deadbeef \
    -D .debug/qemu_debug.log \
    -d int,guest_errors

