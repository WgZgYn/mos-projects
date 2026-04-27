/* =========================================================
 * AP trampoline
 * load addr: 0x8000 (fixed physical address)
 * SIPI vector: load_addr >> 12 = 0x08
 * BOOT DATA: 0x7000 (fixed physical address)
 * BOOT STACK: 0x9000 ~ 0xA000
 * ========================================================= */

.section .text
.code16
# =========================================================
# AP trampoline entry (real mode)
# =========================================================
real_mode_entry:
    cli
    cld

    # 使 CS = 0（SIPI 启动时 CS 可能等于向量号 0x10）
    ljmpw $0x0000, $1f
1:

    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw $0xA000, %sp          # 给个临时栈

    /* ----- 加载 GDT ----- */
    lgdt gdtr32

    /* ----- 开启保护模式 ----- */
    movl %cr0, %eax
    orl  $1, %eax
    movl %eax, %cr0

    /* ----- 远跳转到 32 位代码（动态计算目标地址）----- */
    ljmpl $0x08, $protected_mode_entry

# =========================================================
# 32-bit protected mode
# =========================================================
.code32
protected_mode_entry:
    /* 加载数据段 */
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss
    movl $0xA000, %esp

    # boot_data pointer is passed in memory (fixed location)
    # 0x7000 = struct boot_data
    movl $0x7000, %eax

    # load CR3
    movl (%eax), %ebx
    movl %ebx, %cr3

    # enable PAE
    movl %cr4, %eax
    orl $0x20, %eax
    movl %eax, %cr4

    # enable long mode
    movl $0xC0000080, %ecx
    rdmsr
    orl $0x900, %eax
    wrmsr

    # enable paging
    movl %cr0, %eax
    orl $0x80000000, %eax
    movl %eax, %cr0

    lgdt gdtr64

    ljmp $0x08, $long_mode_entry


# =========================================================
# 64-bit long mode
# =========================================================
.code64
long_mode_entry:
    lgdt gdtr64_full

    movw $0x00, %ax
    movw %ax, %fs
    movw %ax, %gs

    movw $0x10, %ax
    movw %ax, %es
    movw %ax, %ds
    movw %ax, %ss


    movq $0x7000, %rax

    # 1. 设置栈 (确保 16 字节对齐)
    movq 8(%rax), %rsp
    andq $~0xF, %rsp


    movq 24(%rax), %rbx      # 从 BootData 加载 64 位 CR0 值
    movq %rbx, %cr0

    movq 32(%rax), %rbx      # 从 BootData 加载 64 位 CR4 值
    movq %rbx, %cr4

    movq %rax, %rdi          # 将 BootData 传递到 ap_main 函数参数

    # 强制 canonical 跳转
    movq 16(%rax), %rax
    call *%rax

1:
    hlt
    jmp 1b

# ------------------------------------------------------------
#  GDT 表
# ------------------------------------------------------------
.align 16
gdt32:
    # 空描述符
    .quad 0x0000000000000000
    # 32 位代码段：基址 0， 限长 4G，DPL=0，非一致，可读可执行
    .quad 0x00CF9B000000FFFF
    # 32 位数据段：基址 0， 限长 4G，DPL=0，可读写
    .quad 0x00CF93000000FFFF
gdt32_end:

gdtr32:
    .word gdt32_end - gdt32 - 1    # 限长
    .long gdt32                     # 基址（BSP 拷贝后需根据实际地址修补）

.align 16
gdt64:
    # 空描述符
    .quad 0x0000000000000000
    # 64 位代码段：D=0, L=1, 可读可执行
    .quad 0x00AF9B000000FFFF
    # 64 位数据段：兼容模式数据段（实际足够）
    .quad 0x00CF93000000FFFF
gdt64_end:

gdtr64:
    .word gdt64_end - gdt64 - 1
    .long gdt64                     # 同样可能需要 BSP 修补

gdtr64_full:
    .word gdt64_end - gdt64 - 1
    .quad gdt64
