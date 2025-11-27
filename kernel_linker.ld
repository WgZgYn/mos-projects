ENTRY(_start)
OUTPUT_FORMAT("elf64-x86-64")
KERNEL_VMA = 0xffffffff80000000;
SECTIONS {
    . = KERNEL_VMA;

    .text : {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
    }
}
