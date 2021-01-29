uefi-deps = $(shell find ../uefi-rs/ -type f -name '*.rs')
boot_deps = $(shell find ./efi_boot/src/ -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/ -type f -name '*.rs')

bootloader = image/EFI/BOOT/BOOTX64.efi
kernel = image/EFI/gsai/kernel.elf

PROFILE=release

all: $(bootloader) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(kernel)

reset: soft-reset
	cd ./efi_boot/;\
		cargo clean
	cd ./kernel/;\
		cargo clean

	
$(bootloader): $(boot_deps) $(uefi-deps)
	rm -f $(bootloader)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/efi_boot/;\
		rustfmt **/*.rs;\
		cargo build --profile $(PROFILE) -Z unstable-options

$(kernel): $(kernel_deps) $(uefi-deps)
	rm -f $(kernel)
	echo $(PROFILE)
	cd /media/carl/GitHub/gsai/kernel/;\
		rustfmt **/*.rs;\
		cargo build --profile $(PROFILE) -Z unstable-options
		