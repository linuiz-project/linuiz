<<<<<<< Updated upstream
PROFILE=release

boot_deps = $(shell find ./boot/src/ -type f -name "*.rs")
kernel_deps = $(shell find ./kernel/src/ -type f -name "*.rs")
libstd_deps = $(shell find ./libstd/src/ -type f -name "*.rs")

bootloader = ./.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = ./kernel/ap_trampoline.o
kernel = ./.hdd/image/EFI/gsai/kernel.elf
=======
root = $(shell cd)
bootloader = $(root)/.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = $(root)/kernel/ap_trampoline.o
ap_trampoline_src = $(root)/kernel/src/ap_trampoline.asm
kernel = $(root)/.hdd/image/EFI/gsai/kernel.elf
>>>>>>> Stashed changes


all: $(bootloader) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(ap_trampoline) $(kernel)

reset: soft-reset
<<<<<<< Updated upstream
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

$(kernel): $(ap_trampoline) $(libstd_deps) $(kernel_deps)
	cd ./kernel;\
		cargo fmt;\
		cargo build --profile $(PROFILE) -Z unstable-options
	rm -f $(ap_trampoline)
=======
	cd $(root)/boot/ && cargo clean
	cd $(root)/kernel/ && cargo clean
	cd $(root)/libstd/ && cargo clean

update:
	cd $(root)/boot/ && cargo update
	cd $(root)/kernel/ && cargo update
	cd $(root)/libstd/ && cargo update

$(bootloader):
	cd $(root)/boot/ && cargo fmt && cargo build --profile release -Z unstable-options

$(ap_trampoline): $(ap_trampoline_src)
		nasm -f elf64 -o $(ap_trampoline) $(ap_trampoline_src)

$(kernel): $(ap_trampoline)
	cd $(root)/kernel/ && cargo fmt && cargo build --profile release -Z unstable-options
>>>>>>> Stashed changes
