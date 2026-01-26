; hello.asm - Simple "Hello, World!" program for Linux x86 (32‑bit)
;
; Assemble:
;   nasm -f elf32 hello.asm
; Link:
;   ld -m elf_i386 -s -o hello hello.o
; Run:
;   ./hello

section .data
    msg     db      'Hello, World!', 0x0a   ; string + newline
    len     equ     $-msg                     ; length of the string

section .text
    global _start

_start:
    ; write(1, msg, len)
    mov     eax, 4          ; sys_write
    mov     ebx, 1          ; file descriptor (stdout)
    mov     ecx, msg        ; pointer to message
    mov     edx, len        ; message length
    int     0x80

    ; exit(0)
    mov     eax, 1          ; sys_exit
    xor     ebx, ebx        ; status = 0
    int     0x80
