extern _startup, __ap_stack_pointers

section .ap_text

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

    ; Serialize pipeline
    cpuid

    ; Set GDT & long-jump to long mode
    lgdt [__gdt.pointer]
    jmp 0x08:longmode

bits 64
longmode:
    cli

    ; Update segment registers  
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; Get APIC ID (this acts as an __ap_stack_pointers index)
    mov eax, 0x1
    cpuid                           ; The APIC ID is in bits 24..32 (exclusive range)
    shr ebx, 24
    and ebx, 0xFF                   ; `ebx` or `bl` now contains the APIC ID, so we chop off the sign-extended bits
    mov eax, 0x8                    ; Native integer width
    mul ebx                         ; `eax` now contains byte offset of APIC ID (relative to `__ap_stack_pointers`)
    add eax, __ap_stack_pointers    ; `eax` now contains the absolute offset of the AP stack pointer

    mov rsp, [eax]

    ; Jump to high-level code
    call _startup


section .ap_data

global __kernel_pml4, __gdt
__kernel_pml4 resd 1
__gdt:
    resq 7
    
    .pointer:
        dw $ - __gdt - 1
        dq __gdt