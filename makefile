uefi-deps = $(shell find ../uefi-rs/ -type f -name '*.rs')
boot_deps = $(shell find ./efi-boot/src/ -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/src/ -type f -name '*.rs')

# RELEASE
bootloader = image/EFI/BOOT/BOOTX64.efi
kernel = image/EFI/gsai/kernel.elf

all: $(bootloader) $(kernel)

reset:
	rm -f $(bootloader) $(kernel)

debug:
	rm -f $(bootloader) $(kernel)

	cd /media/carl/GitHub/gsai/efi-boot/;\
		rustfmt **/*.rs;\
		cargo build -Z unstable-options
	
	cd /media/carl/GitHub/gsai/kernel/;\
		rustfmt **/*.rs;\
		cargo build -Z unstable-options
	
$(bootloader): $(boot_deps) $(uefi-deps)
	rm -f $(bootloader)
	cd /media/carl/GitHub/gsai/efi-boot/;\
		rustfmt **/*.rs;\
		cargo build --release -Z unstable-options

$(kernel): $(kernel_deps) $(uefi-deps)
	rm -f $(kernel)
	cd /media/carl/GitHub/gsai/kernel/;\
		rustfmt **/*.rs;\
		cargo build --release -Z unstable-options
		