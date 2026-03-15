OUTPUT_FORMAT(elf64-x86-64)

PHDRS
{
  text    PT_LOAD FLAGS(5);   /* R X */
  rodata  PT_LOAD FLAGS(4);   /* R   */
  data    PT_LOAD FLAGS(6);   /* R W */
  dynamic PT_DYNAMIC FLAGS(6);
}

SECTIONS
{
  /* shared object must be position independent */
  . = 0x1000;

  /* ---------------- RX ---------------- */

  .text : ALIGN(4K)
  {
    *(.text .text.*)
    *(.plt)
    *(.plt.*)
  } :text

  /* ---------------- R ---------------- */

  . = ALIGN(4K);

  .rodata :
  {
    *(.rodata .rodata.*)
    *(.eh_frame*)
    *(.gcc_except_table*)
  } :rodata

  .rela.dyn :
  {
    *(.rela.dyn)
  } :rodata

  .rela.plt :
  {
    *(.rela.plt)
  } :rodata

  /* ---------------- RW ---------------- */

  . = ALIGN(4K);

  .data :
  {
    *(.data .data.*)
  } :data

  .got :
  {
    *(.got)
  } :data

  .got.plt :
  {
    *(.got.plt)
  } :data

  .dynamic :
  {
    *(.dynamic)
  } :data :dynamic

  .bss (NOLOAD) :
  {
    *(.bss .bss.*)
    *(COMMON)
  } :data
}