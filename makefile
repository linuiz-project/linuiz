## Command-line arguments

PROFILE=release


## Dependencies

bootloader = ./.hdd/image/EFI/BOOT/BOOTX64.efi
bootloader_deps = $(shell find ./boot/ -type f -name "*.rs")

libkernel_deps = $(shell find ./libkernel/ -type f -name "*.rs")

kernel = ./.hdd/image/EFI/gsai/kernel.elf
kernel_deps = $(shell find ./kernel/ -type f -name "*.rs")
kernel_linker_args = ./kernel/x86_64-unknown-none.json ./kernel/x86_64-unknown-none.lds

ap_trampoline_src = ./kernel/src/ap_trampoline.asm
ap_trampoline_out = ./kernel/ap_trampoline.o

hdd = .hdd
nvme_img = $(hdd)/nvme.img
rootfs_img = $(hdd)/rootfs.img

debug = .debug



## Commands

all: $(nvme_img) $(rootfs_img) $(bootloader) $(kernel)

run: all $(debug)
	objdump -d -D .hdd/image/EFI/gsai/kernel.elf > .debug/kernel_disasm
	./run.sh

reset: clean
	rm -f $(bootloader) $(kernel) $(ap_trampoline_out)

rebuild: reset all

clean:
	cd ./boot/ && cargo clean
	cd ./kernel/ && cargo clean
	cd ./libkernel/ && cargo clean

update:
	rustup update
	cd ./boot/ && cargo update
	cd ./kernel/ && cargo update
	cd ./libkernel/ && cargo update


## Dependency paths

$(bootloader): $(bootloader_deps) $(libkernel_deps) 
	cd ./boot/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options

$(ap_trampoline_out): $(ap_trampoline_src)
	nasm -f elf64 -o $(ap_trampoline_out) $(ap_trampoline_src)

$(kernel): $(ap_trampoline_out) $(kernel_deps) $(libkernel_deps) $(kernel_linker_args)
	cd ./kernel/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options

$(nvme_img): $(hdd)
	qemu-img create -f raw $(nvme_img) 256M

$(rootfs_img): $(hdd)
	qemu-img create -f raw $(rootfs_img) 256M 

$(hdd):
	mkdir $(hdd)

$(debug):
	mkdir $(debug)