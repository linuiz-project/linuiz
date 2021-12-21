boot_deps = $(shell find ./efi_boot/src/ -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/ -type f -name '*.rs')
libkernel_deps = $(shell find ./libkernel/ -type f -name '*.rs')

bootloader = ./hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = ./kernel/ap_trampoline.o
kernel = ./hdd/image/EFI/gsai/kernel.elf

PROFILE=release

all: $(bootloader) $(ap_trampoline) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(ap_trampoline) $(kernel)

reset: soft-reset
	cd ./efi_boot/;\
		cargo clean
	cd ./kernel/;\
		cargo clean
	cd ./libkernel/;\
		cargo clean

	
$(bootloader): $(boot_deps)
	rm -f $(bootloader)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/efi_boot/;\
		rustfmt **/*.rs;\
		cargo build --profile $(PROFILE) -Z unstable-options

$(ap_trampoline): ./kernel/src/ap_trampoline.asm
		nasm -f elf64 -o $(ap_trampoline) ./kernel/src/ap_trampoline.asm

$(kernel): $(ap_trampoline) $(libkernel_deps) $(kernel_deps)
	rm -f $(kernel)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/kernel/;\
		rustfmt **/*.rs;\
		rustfmt ../libkernel/**/*.rs;\
		cargo build --profile $(PROFILE) -Z unstable-options;\
		cd ..;\