extern _startup, __ap_stack_pointers

section .ap_trampoline

global __kernel_pml4
__kernel_pml4 dd 0

bits 16
realmode:
    cli
    cld

    ; Enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Set PML4 address
    mov eax, [__kernel_pml4]
    mov cr3, eax

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
    lgdt [__gdt.pointer]
    jmp __gdt.code:longmode

; Access bits
PRESENT        equ 1 << 7
NOT_SYS        equ 1 << 4
EXEC           equ 1 << 3
DC             equ 1 << 2
RW             equ 1 << 1
ACCESSED       equ 1 << 0

; Flags bits
GRAN_4K       equ 1 << 7
; This flag should not be present with LONG_MODE flag.
; They are mutually excuslive.
SZ_32         equ 1 << 6
LONG_MODE     equ 1 << 5

global __gdt.code, __gdt.data, __gdt.tss, __gdt.pointer
    
__gdt:
    .null: equ $ - __gdt
        dq 0
    .code: equ $ - __gdt
        dd 0xFFFF                           ; Limit & Base (low)
        db 0                                ; Base (mid)
        db PRESENT | NOT_SYS | EXEC | RW    ; Access
        db GRAN_4K | LONG_MODE | 0xF        ; Flags
        db 0                                ; Base (high)
    .data: equ $ - __gdt
        dd 0xFFFF                           ; Limit & Base (low)
        db 0                                ; Base (mid)
        db PRESENT | NOT_SYS | RW           ; Access
        db GRAN_4K | SZ_32 | 0xF            ; Flags
        db 0                                ; Base (high)
    .tss: equ $ - __gdt
        dd 0x00000068
        dd 0x00CF8900
    .pointer:
        dw $ - __gdt - 1
        dq __gdt

bits 64
longmode:
    cli

    ; Update segment registers  
    mov ax, __gdt.data
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Configure temporary stack

    ; Get APIC ID (this acts as an __ap_stack_pointers index)
    mov eax, 0x1
    cpuid                           ; The APIC ID is in bits 24..32 (exclusive range)
    shr ebx, 24
    and ebx, 0xFF                   ; `ebx` or `bl` now contains the APIC ID
    mov eax, 0x8                    ; Native integer width
    mul ebx                         ; `eax` now contains byte offset of APIC ID
    add eax, __ap_stack_pointers    ; `eax` now contains the absolute offset of the AP stack pointer
    mov rsp, [eax]

    ; Jump to high-level code
    call _startup