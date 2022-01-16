PROFILE=release

root = $(shell cd)
bootloader_deps = $(shell dir /s /b .\boot\src\*.rs)
kernel_deps = $(shell dir /s /b .\kernel\src\*.rs)
kernel_compile_args = $(root)/kernel/x86_64-unknown-none.json $(root)/kernel/x86_64-unknown-none.lds
libstd_deps = $(shell dir /s /b .\libstd\src\*.rs)
ap_trampoline_src = $(root)/kernel/src/ap_trampoline.asm
hdd = .hdd
nvme_img = $(hdd)/nvme.img
rootfs_img = $(hdd)/rootf.img

bootloader = $(root)/.hdd/image/EFI/BOOT/BOOTX64.efi
ap_trampoline = $(root)/kernel/ap_trampoline.o
kernel = $(root)/.hdd/image/EFI/gsai/kernel.elf

PROFILE=release

all: $(nvme_img) $(rootfs_img) $(bootloader) $(kernel) 

run: all
	run.bat

soft-reset: 
	rm -fs $(bootloader) $(ap_trampoline) $(kernel)

clean:
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

$(ap_trampoline): $(ap_trampoline_src)
	'C:\Program Files\NASM\nasm.exe' -f elf64 -o $(ap_trampoline) $(ap_trampoline_src)

$(kernel): $(ap_trampoline) $(kernel_deps) $(kernel_compile_args) $(libstd_deps)
	cd $(root)/kernel/ && cargo fmt && cargo build --profile $(PROFILE) -Z unstable-options

$(nvme_img): $(hdd)
	qemu-img.exe create -f raw $(nvme_img) 256M

$(rootfs_img): $(hdd)
	qemu-img.exe create -f raw $(rootfs_img) 256M 

$(hdd):
	mkdir $(hdd)