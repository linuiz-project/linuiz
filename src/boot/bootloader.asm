; remark: the first 512 bytes of the bootloader are reserved for the initial bootsector
;   so! as a rule, be very careful with adding or including code in this file.
;
;   * nasm will however error when it is attempts to compile with negative pad because the size is too large

; — NASM directives —
bits 16
org 0x7C00
global _start

kernel_offset equ 0x1000

; — code —

 _start:
    mov [boot_disk], dl    ; store boot disk

    xor ax, ax      ; zero register
    mov ds, ax      ; zero data segment register
    mov ss, ax      ; stack stars at 0
    mov bp, 0x7C00  ; move stack safely out of the way
    mov sp, bp

    cld ; allow i.e. lodsb to travel forwards

    ; display boot string
    mov si, booting_real_string
    call println
    
    call load_kernel

    ; enter protected mode
    call enter_32bit_protected


enter_32bit_protected:
    cli     ; disable interrupts (they must later be reenabled)
    lgdt [gdt_descriptor]   ; loads the GDT

    ; assign control register bit to enable protected mode
    mov eax, cr0    ; move control register into `eax`
    or eax, 0x1     ; set protected bit
    mov cr0, eax    ; move `eax` back into the control register

    ; we must execute a far jump to force the CPU to flush its pipeline
    ; (and thus, not accidentally execute any code in 16bit real mode)
    jmp code_segment:initialize_protected_mode
    ; we're now in 32bit protected mode

load_kernel:
    ; print status to screen
    mov bx, load_kernel_string 
    call println

    mov bx, kernel_offset   ; set up parameters for disk loading, so that
    mov dh, 2               ; when we kernel sectors into memory (excluding
    mov dl, [boot_disk]     ; the boot sector) from the boot disk to `kernel_offset`
    call disk_read

    ret

; assumes `db:si` already points to string address
; remark: does not return carriage, or similar functionality
println:
    lodsb               ; load next byte
    or al, al           ; compare to store result in flags register
    jnz println_loop    ; if it's not 0 (end of string), print character
    ret                 ; else return
                     
    println_loop:
        call print
        jmp println

; assumes the ASCII value is in the `al` register
print:
    mov ah, 0x0E    ; tell BIOS we want to print one character
    mov bh, 0x00    ; page number
    mov bl, 0x07    ; text attribute 0x07 is a light-gray font on black background
    int 0x10        ; video interrupt, for printing `al` register value as ASCII
    ret

hexprint:
    push dx

    mov si, 0x4 ; set si to the number of bytes in our hex string (after the '0x')
    
    ; loops, writing to `hex_template` until our offset is 0.
    hexwrite_loop:
        cmp si, 0x0         ; compare our offset with zero
        jz exit_hexwrite    ; if zero, print `hex_template` and exit

        call hexwrite       ; write bottom byte into `hex_template`
        sub si, 0x2         ; decrement offset by 2 (for 2 hex characters)
        shr dx, 8           ; shift our printing value to remove minimal byte
        jmp hexwrite_loop   ; loop

    exit_hexwrite:
        mov si, hex_template    ; move `hex_template` to `si` for printing
        call println            ; print `si` register
        pop dx 
        ret

; takes number in `dx` register and
; writes it (as hex) into `hex_template`
hexwrite:
    push bx

    mov bx, hex_ref_table
    mov ax, dx
    
    mov ah, al      ; make `ah` and `al` equal to we can isolate each half of the byte
    shr ah, 4       ; `ah` has high nibble
    and al, 0x0F    ; `al` has low nibble
    xlat            ; lookup al's contents in our table
    xchg  ah, al    ; flip around the bytes so now we can get the higher nibble 
    xlat            ; look up what we just flipped  

    ; move `ax` (which has our two character bytes) into the hex template with offset in `si`
    mov [hex_template + si], ax

    pop bx    
    ret

disk_read:
    push dx         ; push `dx` to stack so we can recall how many 
                    ; sectors we intended to read, even if it is altered

    mov ah, 0x02    ; BIOS read sector function
    mov al, dh      ; read `dh` # of sectors
    mov ch, 0x00    ; select cylinder 0
    mov dh, 0x00    ; select head 0
    mov cl, 0x02    ; start reading from second sector (i.e. after boot sector)
    int 0x13        ; BIOS interrupt

    jc disk_error   ; jump if carry flag is set (i.e. disk read error)

    pop dx          ; restore dx from stack
    cmp dh, al      ; if `al` (sectors read) != `dh` (sectors requested) then
    jne disk_error  ;   display error message
    ret

disk_error:
    mov si, disk_error_string
    call println
    hlt


; — data —
boot_disk                   db 0 

booting_real_string         db 'Successfully booted into real mode... ', 0
load_kernel_string          db 'Loading kernel image... ', 0
disk_error_string           db 'Failed to read disk. ', 0
hex_ref_table               db '0123456789ABCDEF', 0
hex_template                db '0x0000', 0



; — includes —
%include "src/boot/boot16_gdt.asm"

; — end (512-byte padding + magic number) —
times 510 - ($ - $$) db 0   ; pad binary with zero bytes up to 510 (remaining two bytes are magic number)
dw 0xAA55                   ; magic boot signature
                            ;   remark: this number tell the BIOS that the preceding
                            ;   512 bytes are a bootloader, and not random code.

%include "src/boot/boot32.asm"
