uefi-deps = $(shell find ../uefi-rs/ -type f -name '*.rs')
boot_deps = $(shell find ./efi_boot/src/ -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/ -type f -name '*.rs' -o -name '*.asm')
libkernel_deps = $(shell find ./libkernel/ -type f -name '*.rs')

bootloader = ./hdd/image/EFI/BOOT/BOOTX64.efi
kernel = ./hdd/image/EFI/gsai/kernel.elf

PROFILE=release

all: $(bootloader) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(kernel)

reset: soft-reset
	cd ./efi_boot/;\
		cargo clean
	cd ./kernel/;\
		cargo clean
	cd ./libkernel/;\
		cargo clean

	
$(bootloader): $(boot_deps) $(uefi-deps)
	rm -f $(bootloader)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/efi_boot/;\
		rustfmt **/*.rs;\
		cargo build --profile $(PROFILE) -Z unstable-options

$(kernel): $(uefi-deps) $(libkernel_deps) $(kernel_deps)
	rm -f $(kernel)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/kernel/;\
		rustfmt **/*.rs;\
		rustfmt ../libkernel/**/*.rs;\
		nasm -f elf64 -o ap_trampoline.o ./src/ap_trampoline.asm;\
		cargo build --profile $(PROFILE) -Z unstable-options;\
		rm -f ap_trampoline.o;\