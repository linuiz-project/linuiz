OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_entry)

PHDRS {
    null        PT_NULL     FLAGS(0x0);
    text        PT_LOAD     FLAGS(0x1);
    rodata      PT_LOAD     FLAGS(0x4);
    data        PT_LOAD     FLAGS(0x6);
    dynamic     PT_DYNAMIC  FLAGS(0x6);
}

SECTIONS
{
    /* Place kernel in the last 2GB of virtual memory. */
    . = 0xffffffff80200000;



    .text : ALIGN(0x1000) {
        __text_start = .;

        __ap_text_start = .;
        KEEP(*(.ap_text*))
        __ap_text_end = .;
        
        *(.text*)

        __text_end = .;
    } :text



    .rodata : ALIGN(0x1000) {
        __rodata_start = .;

        *(.rodata*)

        __rodata_end = .;
    } :rodata

    .stivale2hdr : { KEEP(*(.stivale2hdr)) } :rodata



    .data : ALIGN(0x1000) {
        __data_start = .;

        __ap_data_start = .;
        KEEP(*(.ap_data*))
        __ap_data_end = .;

        *(.data*)

        __data_end = .;
    } :data

    .bss : ALIGN(0x1000) {
        __bss_start = .;

        /* Make sure local state is page-aligned. */
        . = ALIGN(0x1000);  
        __local_state_start = .;
        . += 0x1000;
        __local_state_end = .;

        *(.bss*)

        __bss_end = .;
    } :data

    .dynamic : { *(.dynamic) } :data :dynamic

    /* 
    .user_code : ALIGN(0x1000) {
        __user_code_start = .;

        *(.user_code*)

        __user_code_end = .;
    }
    */
}