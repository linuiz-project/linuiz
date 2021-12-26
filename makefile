PROFILE=release

boot_deps = $(shell find ./boot/src/ -type f -name "*.rs")
kernel_deps = $(shell find ./kernel/src/ -type f -name "*.rs")
libkernel_deps = $(shell find ./libstd/src/ -type f -name "*.rs")

bootloader = ./.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = ./kernel/ap_trampoline.o
kernel = ./.hdd/image/EFI/gsai/kernel.elf


all: $(bootloader) $(ap_trampoline) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(ap_trampoline) $(kernel)

reset: soft-reset
	cd ./boot/;\
		cargo clean
	cd ./kernel/;\
		cargo clean
	cd ./libstd/;\
		cargo clean

	
$(bootloader): $(boot_deps)
	cd ./boot;\
		cargo fmt;\
		cargo build --profile $(PROFILE) -Z unstable-options

$(ap_trampoline): ./kernel/src/ap_trampoline.asm
		nasm -f elf64 -o $(ap_trampoline) ./kernel/src/ap_trampoline.asm

$(kernel): $(ap_trampoline) $(libkernel_deps) $(kernel_deps)
	cd ./kernel;\
		cargo fmt;\
		cargo build --profile $(PROFILE) -Z unstable-options