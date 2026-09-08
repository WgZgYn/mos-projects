# Roadmap: 让 MOS 内核运行静态 musl + BusyBox

> 目标:内核 ABI 与 Linux x86_64 兼容(至少覆盖静态 musl 所需子集),
> 可以交叉编译并运行静态链接的 BusyBox,提供可交互的 shell。
>
> 创建于 2026-09-08。执行状态见下方勾选项。

## 总体原则

1. **syscall 号严格遵循 Linux x86_64 表**——musl 按 Linux 号直呼 syscall,号错一切皆错。
2. **返回值遵循 `-errno` 约定**(rax ∈ [-4095, -1] 表示错误),不再返回 `u64::MAX`。
3. **未知 syscall 返回 `-ENOSYS`**,绝不杀死进程。
4. **全部静态链接**。放弃内核内朴素动态链接器(elf.rs 只支持 3 种 reloc,
   无 TLS/IFUNC/COPY,跑不了真 musl 动态库)。busybox 静态编译是常规做法。
5. **二进制 ABI 结构(stat / dirent64 / utsname / sigframe / auxv)必须与 Linux 一致**,
   musl 直接按 C 结构体解析这些内存布局。

## 阶段 1:ABI 地基 —— 静态 musl hello world 能跑

- [x] 修复 syscall 号冲突:GetCpu(39→自定义号)、Rand(7→自定义号)、Sleep(62→35 nanosleep)、GetDents(78→217 getdents64);自定义 syscall(信号量/线程/窗口/时钟)整体挪到 1000+ 区间
- [x] 错误约定:`-errno` 替换 `u64::MAX`;同步更新 user_lib 的判断逻辑
- [x] 未知 syscall → `-ENOSYS`(替换现在的 `sys_exit(-1)`)
- [x] 基础进程 syscall:`exit_group`(231)、`set_tid_address`(218)、`getpid`(39)、`getppid`(110)、`getuid/geteuid/getgid/getegid`(102/107/104/108,返回 0)、`uname`(63)
- [x] `arch_prctl`(158,ARCH_SET_FS/GET_FS):TLS 线程指针;上下文切换保存/恢复 FS base
- [x] 文件 syscall:`lseek`(8)、`fstat`(5)、`stat`(4)、`lstat`(6)、`getdents64`(217,Linux dirent64 布局,替换自定义 UserDirEntry)
- [x] 时间 syscall:`clock_gettime`(228,CLOCK_MONOTONIC/REALTIME)、`gettimeofday`(96)、`nanosleep`(35,timespec 语义);wall clock 从 RTC/CMOS 初始化
- [x] `mprotect`(10)实现、`madvise`(28)stub 返回 0
- [x] execve 写辅助向量 auxv:AT_PHDR/AT_PHENT/AT_PHNUM/AT_PAGESZ/AT_ENTRY/AT_RANDOM/AT_UID/AT_GID/AT_CLKTCK(静态 musl 的 TLS 初始化依赖 AT_PHDR)
- [x] execve 失败返回 `-errno`(替换 `sys_exit(-1)`)
- [x] user_lib 同步适配新 syscall 号与错误约定,全部现有测试通过
- [x] 验证:交叉编译静态 musl hello world,在 QEMU 中运行成功(2026-09-08,输出 hello/uname/time 全部正确)

> 阶段 1 验证中额外修复的内核 bug(均为真实 Linux 兼容性缺陷):
> 1. execve 栈布局:argv/envp/auxv 顺序错误,rsp 指向 auxv 而非 argc;已重排为 Linux 布局(低→高:argc/argv/NULL/envp/NULL/auxv/AT_RANDOM 16B/字符串),AT_RANDOM 不再被 auxv 覆盖
> 2. AT_PHDR 计算:e_phoff 是文件偏移,固定加载时 phdr 应为 `p_vaddr + (e_phoff - p_offset)`(之前算成 0x40)
> 3. ELF 加载器缺 padzero:最后一页 filesz 之后的尾部未清零,.bss 读到文件尾随数据(.comment 段)——musl 的 main_tls 读成垃圾导致 mmap 巨大失败;已加只对可写段清零

## 阶段 2:文件系统补全 —— busybox ls/cat/cp/mv 能跑

- [x] `mkdir`(83)、`unlink`(87)、`rmdir`(84)、`rename`(82)、`link`(86)
- [x] `symlink`(88)、`readlink`(89)
- [x] `access`(21)/`faccessat`(269)、`chmod/fchmod`(90/91)、`umask`(95)、`truncate/ftruncate`(76/77)
- [x] `readv/writev`(19/20)、`fcntl`(72:F_DUPFD/F_GETFD/F_SETFD/F_GETFL/F_SETFL)、`dup3`(292)
- [x] 修复 fork 跨 CPU TLB 刷新(smp fix plan.md 中 P0-2)
- [x] 验证:静态 busybox(musl-gcc 1.36.1)在 QEMU 中运行 echo/ls/cat

> 阶段 2 验证中额外修复的内核 bug:
> 1. mmap 分发把 `align_up(len,4096)` 字节值当页数用 → 4KB mmap 占 16MB(已修 mmap/munmap 两处)
> 2. mmap MAP_FIXED 与 brk 区域重叠直接 ENOMEM → musl malloc 堆预留失败报 out of memory;已按 Linux 语义实现覆盖已有映射(remove_range + PTE 卸载)
> 3. brk 成功返回 0 而非新 brk → musl malloc 判失败;已改为 Linux 约定
> 4. getdents64 不追踪目录偏移 → busybox 读目录死循环;已用 File offset 追踪
> 5. ramfs 硬链接条目沿用 inode 名字(应为新名字)
> 6. 内核日志:ConsoleLogger 注册后 info! 只进屏幕不进串口(mos/src/main.rs 已注释,调试期用默认 BOTH_LOGGER)
> 7. init 进程只创建 fd 0/1,没有 fd 2 → busybox 的 stderr 写入 EBADF 静默丢失(待补)

## 阶段 3:信号机制 + wait4 语义 —— busybox sh 可交互

- [ ] per-process 信号结构:pending 位图、阻塞掩码、handler 表(fork 继承,execve 清 handler 保掩码)
- [ ] 投递:内核→用户返回路径上检查 pending,构造 Linux rt_sigframe,rax=rt_sigreturn
- [ ] syscall:`rt_sigaction`(13)、`rt_sigprocmask`(14)、`rt_sigreturn`(15)、`kill`(62)、`tkill`(200)
- [ ] 信号源:SIGCHLD(子进程退出)、SIGPIPE(写关闭管道)、SIGINT(键盘 Ctrl-C→前台)、`setitimer`(38)→SIGALRM
- [ ] wait4 语义补全:`pid=-1` 等任意子进程、WNOHANG、状态编码 `(code<<8)|sig`、僵尸进程回收
- [ ] 验证:busybox sh 交互运行、kill applet、^C 中断前台进程

## 阶段 4:shell 体验完善 —— 完整 busybox sh + 大多数 applet

- [ ] `poll`(7)/`select`(23)——ash `read -t`、`timeout`
- [ ] `ioctl`(16):tty 上至少 TCGETS/TIOCGWINSZ(否则 busybox 认为无 tty,退化为非交互 shell)
- [ ] `getrlimit/setrlimit`(97/160,stub 合理默认)、`sysinfo`(99)、`times`(100)
- [ ] `getrandom`(318,musl getentropy 用,接现有 RNG)
- [ ] wall clock 完整:`date`、`ls -l` 时间戳、inode mtime
- [ ] 验证:busybox sh 有提示符、行编辑、`free`、`timeout`、`stty` 等可用

## 阶段 5:扩展(非阻塞主线)

- [ ] `clone`(56)+ `futex`(202):pthread 多线程
- [ ] `setpgid/setsid/getpgrp`(109/112/111):作业控制
- [ ] 最小 `/proc`(busybox ps/free/mount)
- [ ] TCP/IP 网络栈(ping/wget 等网络 applet)
- [ ] 持久化文件系统(ext2/fat)
- [ ] 动态链接(需完整 reloc + TLS + IFUNC,工作量巨大,收益低)

## 关键文件索引

| 内容 | 位置 |
|---|---|
| syscall 号定义 | `mos_abi/src/syscall.rs` |
| syscall 分发 | `mos_syscall/src/lib.rs` |
| 进程管理(fork/exec/wait/exit) | `mos_syscall/src/process.rs` |
| ELF 加载器 | `mos_syscall/src/task/elf.rs` |
| PCB / 文件表 | `mos_task/src/process.rs` |
| COW fork / mmap | `mos_mem/src/virt.rs` |
| 缺页处理(COW 解析) | `mos_runtime/src/idt.rs` |
| 用户库(syscall 封装) | `user/user_lib/src/` |
| 错误码 | `mos_abi/src/error.rs` |
| 根文件系统构建 | `ramfs_builder/` |

## 已识别的坑(来自差距分析)

1. `mos_syscall/src/lib.rs:201-204` 未知 syscall 走 `sys_exit(-1)`——Clone/Mkdir 未分发,一调就死进程
2. `GetCpu=39` 占用 Linux getpid 号;`Rand=7` 占用 poll;`Sleep=62` 占用 exit_group
3. 全内核错误返回 `u64::MAX` 而非 `-errno`
4. `getdents` 用自定义 `UserDirEntry{name[32],type,size}` 布局,musl 解析不了
5. `sys_wait` 只能等指定 pid 且永远阻塞,无 WNOHANG/无 -1/退出状态未编码
6. ~~execve 无 auxv,静态 musl 的 TLS 初始化会崩溃~~ 已修复(阶段 1)
7. ~~execve 失败直接 `sys_exit(-1)`(mos_syscall/src/process.rs:110),应返回 -errno~~ 已修复(阶段 1)
8. 无 wall clock(RTC/CMOS 未读),`date`、`ls -l` 时间戳缺失
9. fork 后 COW 页跨 CPU TLB 未刷新(详见 smp fix plan.md P0-2)
10. 内核内动态链接器只有 RELATIVE/GLOB_DAT/JUMP_SLOT 三种 reloc,放弃,走静态
