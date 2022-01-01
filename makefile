PROFILE=release

root = $(shell cd)
bootloader_deps = $(shell dir /s /b .\boot\src\*.rs)
kernel_deps = $(shell dir /s /b .\kernel\src\*.rs)
libstd_deps = $(shell dir /s /b .\libstd\src\*.rs)

bootloader = $(root)/.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = $(root)/kernel/ap_trampoline.o
kernel = $(root)/.hdd/image/EFI/gsai/kernel.elf

PROFILE=release

all: $(bootloader) $(kernel)

run: all
	run.bat

soft-reset: 
	rm -fs $(bootloader) $(ap_trampoline) $(kernel)

reset:
	cd $(root)/boot/ && cargo clean
	cd $(root)/kernel/ && cargo clean
	cd $(root)/libstd/ && cargo clean

update:
	rustup update
	cd $(root)/boot/ && cargo update
	cd $(root)/kernel/ && cargo update
	cd $(root)/libstd/ && cargo update

$(bootloader): $(bootloader_deps)
	cd $(root)/boot/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options

$(ap_trampoline): $(root)/kernel/src/ap_trampoline.asm
		nasm -f elf64 -o $(ap_trampoline) $(ap_trampoline_src)

$(kernel): $(ap_trampoline) $(kernel_deps) $(libstd_deps)
	cd $(root)/kernel/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options
