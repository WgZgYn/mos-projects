as --64 -o trampoline.o trampoline.asm
ld -m elf_x86_64  -Ttext=0x8000    -N    --oformat binary    -o trampoline.bin    trampoline.o