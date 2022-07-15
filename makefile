## Command-line arguments

PROFILE=release


## Dependencies

bootloader = ./.hdd/image/EFI/BOOT/BOOTX64.EFI
bootloader_deps = $(shell find ./limine/common/ -type f -name "*")

liblz_deps = $(shell find ./liblz/ -type f -name "*.rs")

kernel = ./.hdd/image/EFI/linuiz/kernel.elf
kernel_deps = $(shell find ./kernel/ -type f -name "*.rs")
kernel_linker_args = ./kernel/x86_64-unknown-none.json ./kernel/x86_64-unknown-none.lds

hdd = .hdd
nvme_img = $(hdd)/nvme.img
rootfs_img = $(hdd)/rootfs.img

debug = .debug



## Commands

all: $(nvme_img) $(rootfs_img) $(bootloader) $(kernel)
	mkdir -p .debug
	objdump -d -D .hdd/image/linuiz/kernel.elf > .debug/kernel_disasm

run: all $(debug)
	./run.sh

reset: clean
	rm -f $(bootloader) $(kernel)

rebuild: reset all

clean:
	cd ./kernel/ && cargo clean
	cd ./liblz/ && cargo clean

update:
	cd ./limine/ && git pull
	rustup update
	cd ./kernel/ && cargo update
	cd ./liblz/ && cargo update


## Dependency paths

$(bootloader): ./resources/BOOTX64.EFI ./resources/limine.cfg
	mkdir -p ./.hdd/image/EFI/BOOT/
	cp ./resources/BOOTX64.EFI ./.hdd/image/EFI/BOOT/
	cp ./resources/limine.cfg ./.hdd/image/EFI/BOOT/

$(kernel): $(kernel_deps) $(liblz_deps) $(kernel_linker_args)
	cd ./kernel/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options
	objdump -D .hdd/image/linuiz/kernel.elf > .debug/kernel_disasm

$(nvme_img): $(hdd)
	qemu-img create -f raw $(nvme_img) 256M

$(rootfs_img): $(hdd)
	qemu-img create -f raw $(rootfs_img) 256M 

$(hdd):
	mkdir $(hdd)

$(debug):
	mkdir $(debug)