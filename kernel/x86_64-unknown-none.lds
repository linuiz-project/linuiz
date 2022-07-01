OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_entry)

KERN_BASE = 0xffffffff80000000;
BSP_STACK_SIZE = 0x16000;

SECTIONS
{
    /* Place kernel in the last 2GB of virtual memory. */
    . = KERN_BASE + SIZEOF_HEADERS; 

    .hash               : { *(.hash) }
    .gnu.hash           : { *(.gnu.hash) }
    .dynsym             : { *(.dynsym) }
    .dynstr             : { *(.dynstr) }
    .rela.dyn           :
    {
        *(.rela.dyn)
        *(.rela.text    .rela.text.*)
        *(.rela.rodata  .rela.rodata.*)
        *(.rela.data.rel.ro*)
        *(.rela.got)
        *(.rela.data    .rela.data.*)
        *(.rela.tdata   .rela.tdata.*)
        *(.rela.tbss    .rela.tbss.*)
        *(.rela.bss     .rela.bss.*)
        *(.rela.ifunc)
    }
    .rela.plt           : { *(.rela.plt) *(.rela.iplt) }

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __code_start        = .;
    .plt                : { *(.plt) *(.iplt) }
    .text               : { *(.text .text.*) }
    __code_end          = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __ro_start          = .;
    .rodata             : { *(.rodata .rodata.*) }
    .eh_frame_hdr       : { *(.eh_frame_hdr) }
    __ro_end            = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __rw_start          = .;
    /* relro sections */
    .eh_frame           : { KEEP(*(.eh_frame)) }
    .gcc_except_table   : { KEEP(*(.gcc_except_table)) }
    .tdata              : { *(.tdata .tdata.*) }
    .tbss               : { *(.tbss .tbss.*) }
    .dynamic            : { *(.dynamic) }
    .got                : { *(.got) *(.igot) }
    .                   = ALIGN(CONSTANT(MAXPAGESIZE));
    .                   = DATA_SEGMENT_RELRO_END(0, .);
    /* data sections */
    .got.plt            : { *(.got.plt) *(.igot.plt) }
    .data               : { *(.data .data.*) }
    .bss                : { *(.bss .bss.*) }
    /* bsp stack */
    .                   = ALIGN(0x10);
    .                   += BSP_STACK_SIZE;
    __bsp_top           = .;
    __rw_end            = .;
    .                   = DATA_SEGMENT_END(.);

    .comment              0 : { *(.comment) }
    .debug                0 : { *(.debug) }
    .debug_abbrev         0 : { *(.debug_abbrev) }
    .debug_aranges        0 : { *(.debug_aranges) }
    .debug_frame          0 : { *(.debug_frame) }
    .debug_funcnames      0 : { *(.debug_funcnames) }
    .debug_info           0 : { *(.debug_info .gnu.linkonce.wi.*) }
    .debug_line           0 : { *(.debug_line) }
    .debug_loc            0 : { *(.debug_loc) }
    .debug_macinfo        0 : { *(.debug_macinfo) }
    .debug_pubnames       0 : { *(.debug_pubnames) }
    .debug_pubtypes       0 : { *(.debug_pubtypes) }
    .debug_ranges         0 : { *(.debug_ranges) }
    .debug_sfnames        0 : { *(.debug_sfnames) }
    .debug_srcinfo        0 : { *(.debug_srcinfo) }
    .debug_str            0 : { *(.debug_str) }
    .debug_typenames      0 : { *(.debug_typenames) }
    .debug_varnames       0 : { *(.debug_varnames) }
    .debug_weaknames      0 : { *(.debug_weaknames) }
    .line                 0 : { *(.line) }
    .shstrtab             0 : { *(.shstrtab) }
    .strtab               0 : { *(.strtab) }
    .symtab               0 : { *(.symtab) }
}