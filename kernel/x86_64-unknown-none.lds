ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
    . = 0x7A12000;

    .text : ALIGN(0X1000) {
        _text_start = .;

        *(.text.*)
        *(.text)

        _text_end = .;
    }

    .rodata : ALIGN(0x1000) {
        _rodata_start = .;

        *(.rodata.*)
        *(.rodata)

        _rodata_end = .;
    }

    .data : ALIGN(0x1000) {
        _data_start = .;

        *(.data.*)
        *(.data)

        _data_end = .;
    }

    .bss : ALIGN(0x1000) {
        _bss_start = .;

        *(.bss.*)
        *(.bss)

        _bss_end = .;
    }
}