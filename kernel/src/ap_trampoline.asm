section .bss

extern _ap_startup
global __cpu_init_complete, __kernel_pml4, __stack
__cpu_init_complete  resb 1
__kernel_pml4        resd 1
__stack              resb 4096


section .ap_trampoline

global __gdt_code, __gdt_data, __gdt_tss, __gdt_pointer
__gdt_code           resw 1
__gdt_data           resw 1
__gdt_tss            resw 1
__gdt_pointer        resq 1

bits 16
realmode:
    cli
    cld

    ; Enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Set PML4 address
    or eax, __kernel_pml4

    ; Enable long mode
    mov ecx, 0xC0000080     ; Set correct MSR
    rdmsr
    or eax, 1 << 8          ; Set LME bit
    wrmsr

    ; Set PME & PGE bits
    mov eax, cr0
    or eax, 1 << 31 | 1 << 0
    mov cr0, eax

    ; Set GDT & long-jump to long mode
    lgdt [__gdt_pointer]
    jmp __gdt_code:longmode

bits 64
longmode:
    cli

    ; Configure temporary stack
    mov rsp, __stack

    ; Jump to high-level code
    call _ap_startup