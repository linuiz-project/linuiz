all: gsai_os.img

gsai_os.img: src/boot/bootloader.bin kernel.bin
	cat $^ > $@

	# pad file to 4kb
	dd if=/dev/zero of=$@ bs=1 count=1 seek=4095
	
	# cleanup intermediate build files
	find ./src/ -type f -name '*.o' -delete
	find ./ -type f -name '*.bin' -delete

# build kernel binary
# - kernel_entry jumps to kernel_main()
# - the compiled C kernel
kernel.bin: src/kernel_entry.o target/i686-unknown-linux-gnu/release/libkernel.a
	ld -Ttext 0x1000 --oformat=binary -m elf_i386 $^ -o $@

target/i686-unknown-linux-gnu/release/libkernel.a:
	cargo build --release

# generic rule for building `.o` from `.asm`
%.o: %.asm
	nasm $< -f elf -o $@

%.bin: %.asm
	nasm $< -f bin -o $@


# clean up intermediate make'd files
clean:
	rm -fr *.dis *.bin *.o *.map
	rm -fr kernel/*.o boot/*.bin drivers/*.o