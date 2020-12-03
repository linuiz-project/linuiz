bits 32

initialize_protected_mode:
    jmp $
    ; update segment register to point to data segment
    mov ax, data_segment
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov ebp, 0x90000    ; update stack position so it's right
    mov esp, ebp        ; at the top of the free space

    call protected_start

protected_start:
    ; clear display
    call clear_display

    mov ebx, booting_protected_string
    call println32
    
    ; call query_cpuid_support

    call kernel_offset  ; enter kernel

    jmp $

clear_display:
    pusha
    mov si, 0x7D0 ; hex 2000, which is 80 * 25 (VGA buffer size)
    mov edx, video_memory ; set `edx` to the start of video memory

    clear_loop:
        mov al, 0   ; move the character to the `al` register
        mov ah, white_on_black_attrib   ; set the output color attribute

        cmp si, 0           ; if `al` is zero (end of string),
        je clear_done   ;   then jump done

        mov [edx], ax   ; store character and attributes at current cell
        add ebx, 1      ; increment `ebx` to the next character
        add edx, 2      ; increment `edx` to next video memory character (2 bytes)
        dec si          ; decrement si
        jmp clear_loop  ; loop

    clear_done:
        popa
        ret


println32:
    pusha
    mov edx, video_memory ; set `edx` to the start of video memory

    println32_loop:
        mov al, [ebx]                   ; move the character to the `al` register
        mov ah, white_on_black_attrib   ; set the output color attribute

        cmp al, 0           ; if `al` is zero (end of string),
        je println32_done   ;   then jump done

        mov [edx], ax   ; store character and attributes at current cell
        add ebx, 1      ; increment `ebx` to the next character
        add edx, 2      ; increment `edx` to next video memory character (2 bytes)
        jmp println32_loop  ; loop

    println32_done:
        popa
        ret

query_cpuid_support:
    pushfd  ; copy flags in to `eax` via stack
    pop eax

    mov ecx, eax        ; copy to `ecx` as well for comparison later
    xor eax, 1 << 21    ; flip the ID bit

    push eax    ; copy `eax` via flags to the stack
    popfd

    push ecx    ; retore flags from the old version stored
    popfd       ; in `ecx` (i.e. flipping the ID bit back if 
                ; it was ever flipped)
    
    xor eax, ecx        ; compare `eax` and `ecx`.
    jz no_cpuid_support ; if they are equal then that means the bit
                        ; was not flipped, and cpuid is not supported
    ret

    no_cpuid_support:
        mov ebx, no_cpuid_support_string
        call println32
        hlt

; — data —
video_memory equ 0xB8000
white_on_black_attrib equ 0x0F

booting_protected_string        db 'Elevated to 32-bit protected mode.', 0
no_cpuid_support_string         db 'CPU does not have CPUID support; cannot boot.'