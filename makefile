boot_deps = $(shell find ./efi-boot/src -type f -name '*.rs')
kernel_deps = $(shell find ./kernel/src -type f -name '*.rs')

all: image/EFI/BOOT/BOOTX64.efi image/EFI/gsai/kernel.elf

image/EFI/BOOT/BOOTX64.efi: $(boot_deps)
	cd /media/carl/GitHub/gsai/efi-boot/;\
		cargo build --release -Z unstable-options

image/EFI/gsai/kernel.elf: $(kernel_deps)
	cd /media/carl/GitHub/gsai/kernel/;\
		cargo build --release -Z unstable-options;\
		mv ../image/EFI/gsai/kernel ../image/EFI/gsai/kernel.elf