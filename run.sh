#!/bin/bash

qemu-system-x86_64 \
    -no-reboot \
    -machine q35 \
    -cpu qemu64 \
    -smp 2 \
    -m 64M \
    -serial mon:stdio \
    -display none \
    -net none \
    -D .debug/qemu_debug.log \
    -d int,guest_errors
