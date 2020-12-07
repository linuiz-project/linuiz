uefi-deps = $(shell find ../uefi-rs/ -type f -name '*.rs')
boot_deps = $(shell find ./efi-boot/src/ -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/src/ -type f -name '*.rs')

bootloader = image/EFI/BOOT/BOOTX64.efi
kernel = image/EFI/gsai/kernel.elf

all: $(bootloader) $(kernel)

$(bootloader): $(boot_deps) $(uefi-deps)
	rm -f $(bootloader)
	cd /media/carl/GitHub/gsai/efi-boot/;\
		cargo build --release -Z unstable-options

$(kernel): $(kernel_deps) $(uefi-deps)
	rm -f $(kernel)
	cd /media/carl/GitHub/gsai/kernel/;\
		cargo build --release -Z unstable-options;\
		mv ../image/EFI/gsai/kernel ../$(kernel)