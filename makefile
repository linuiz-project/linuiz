PROFILE=release

root = $(shell cd)
bootloader = $(root)/.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = $(root)/kernel/ap_trampoline.o
ap_trampoline_src = $(root)/kernel/src/ap_trampoline.asm
kernel = $(root)/.hdd/image/EFI/gsai/kernel.elf


all: $(bootloader) $(kernel)

soft-reset: 
	rm -f $(bootloader) $(ap_trampoline) $(kernel)

reset: soft-reset
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
