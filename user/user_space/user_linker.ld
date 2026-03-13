ENTRY(_start)
OUTPUT_FORMAT("elf64-x86-64")
SECTIONS
{
  /* ELF 基地址 */
  . = 0x400000;

  /* -------- code & rodata -------- */
  .text : ALIGN(4K) {
    *(.text*)
  }

  .rodata : ALIGN(4K) {
    *(.rodata*)
  }

  /* -------- data -------- */
  .data : ALIGN(4K) {
    *(.data*)
  }

  /* -------- bss -------- */
  .bss : ALIGN(4K) {
    *(.bss*)
    *(COMMON)
  }

  /* -------- discard -------- */
  /DISCARD/ : {
    *(.eh_frame*)
    *(.comment*)
  }
}
