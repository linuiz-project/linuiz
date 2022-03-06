ENTRY(_entry)
OUTPUT_FORMAT(elf64-x86-64)

PHDRS {
    text       PT_LOAD FLAGS(0x1);
    rodata     PT_LOAD FLAGS(0x4);
    data       PT_LOAD FLAGS(0x6);
    user_code  PT_LOAD FLAGS(0x1);
}

SECTIONS
{
    .text : ALIGN(0x1000) {
        __text_start = .;

        __ap_text_start = .;
        *(.ap_text.* .ap_text)
        __ap_text_end = .;
        
        *(.text.* .text)

        __text_end = .;
    } : text

    .rodata : ALIGN(0x1000) {
        __rodata_start = .;

        *(.rodata.* .rodata)

        __rodata_end = .;
    } : rodata

    .data : ALIGN(0x1000) {
        __data_start = .;

        __ap_data_start = .;
        *(.ap_data.* .ap_data)
        __ap_data_end = .;

        *(.data.* .data)

        __data_end = .;
    } : data

    .bss : ALIGN(0x1000) {
        __bss_start = .;

        *(.bss.* .bss)

        __bss_end = .;
    } : data

    .user_code : ALIGN(0x1000) {
        __user_code_start = .;

        *(.user_code.* .user_code)

        __user_code_end = .;
    }
}