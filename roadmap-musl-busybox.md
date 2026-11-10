# Roadmap: 让 MOS 内核运行静态 musl + BusyBox

> 目标:内核 ABI 与 Linux x86_64 兼容(至少覆盖静态 musl 所需子集),
> 可以交叉编译并运行静态链接的 BusyBox,提供可交互的 shell。
>
> 创建于 2026-09-08。执行状态见下方勾选项。

## 产品定位与兼容边界

MOS 的近期目标不是在内核中重新实现一份 C 标准库，也不是立即做到完整 Linux
兼容，而是提供一个**可验证的 Linux x86_64 内核 ABI 子集**，使下列程序能够运行：

1. 使用 musl 静态链接的基础 C 程序；
2. 使用 malloc、文件、目录、时间、进程和管道等常用 libc 能力的程序；
3. 静态 BusyBox 及其常用 applet；
4. 最终支持 pthread 所需的 clone/futex/TLS 最小子集。

当前主线明确采用静态链接。动态 ELF、动态 musl、glibc、完整 `/proc`、网络栈和
完整 Linux 驱动 ABI 均不属于“基础 musl/Linux 兼容”里程碑的前置条件。

### 兼容等级

| 等级 | 能力定义 | 当前判断 |
|---|---|---|
| L0：ABI 启动 | 正确 ELF 入口、用户栈、auxv、errno、基础 write/exit | 已完成 |
| L1：基础 musl C | 静态 musl hello、malloc、时间、基础文件 I/O | 已完成，但需自动化回归固化 |
| L2：BusyBox 基础命令 | `echo/ls/cat/cp/mv` 等非交互 applet | 基本完成 |
| L3：交互 shell | wait4、信号、tty、poll/select、Ctrl-C | 进行中 |
| L4：常用 Linux 用户态 | rlimit、sysinfo、getrandom、进程组、最小 procfs | 未开始/部分基础存在 |
| L5：基础 pthread | clone、futex、TLS、exit_group/clear_child_tid 语义 | 未完成 |

每个等级只有在以下条件同时满足时才能标记完成：

- Linux x86_64 syscall 号、结构体布局、errno 和边界行为有明确对照；
- C/musl 探针和对应 BusyBox 场景都有自动化测试；
- 无效用户指针返回 `-EFAULT`，不会造成内核崩溃；
- 声明支持 SMP 时，单核和 4 核回归都通过；
- 结果不是仅依据一次人工串口日志判断。

详细任务分类、依赖和新对话执行文件见
[`docs/tasks/ROADMAP.md`](docs/tasks/ROADMAP.md)。

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
- [x] user_lib 同步适配新 syscall 号与错误约定,当时的现有测试通过(待任务 04/09 固化为自动回归)
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
- [x] fork 后已接入跨 CPU TLB shootdown(smp fix plan.md 中 P0-2)
- [ ] shootdown 增加逐 CPU generation/acknowledgement,并覆盖 mprotect/munmap/MAP_FIXED/地址空间销毁
- [x] 验证:静态 busybox(musl-gcc 1.36.1)在 QEMU 中运行 echo/ls/cat

> 阶段 2 验证中额外修复的内核 bug:
> 1. mmap 分发把 `align_up(len,4096)` 字节值当页数用 → 4KB mmap 占 16MB(已修 mmap/munmap 两处)
> 2. mmap MAP_FIXED 与 brk 区域重叠直接 ENOMEM → musl malloc 堆预留失败报 out of memory;已按 Linux 语义实现覆盖已有映射(remove_range + PTE 卸载)
> 3. brk 成功返回 0 而非新 brk → musl malloc 判失败;已改为 Linux 约定
> 4. getdents64 不追踪目录偏移 → busybox 读目录死循环;已用 File offset 追踪
> 5. ramfs 硬链接条目沿用 inode 名字(应为新名字)
> 6. 内核日志:ConsoleLogger 注册后 info! 只进屏幕不进串口(mos/src/main.rs 已注释,调试期用默认 BOTH_LOGGER)
> 7. init 进程只创建 fd 0/1,没有 fd 2 → busybox 的 stderr 写入 EBADF 静默丢失（2026-09-09 已补 fd 2；单核/4 核回归通过）

## 阶段 3:信号机制 + wait4 语义 —— busybox sh 可交互

> 额外完成(2026-09-09):shell 重写支持参数传递(通用 fork+execve,cd/exit 内置);
> ioctl(16) 的 TCGETS/TIOCGWINSZ 已提前实现(阶段 4 该项完成)——busybox isatty 生效,
> 输出带 ANSI 颜色码(内核控制台暂不支持 ANSI,串口可见原始转义,待阶段 4 处理)。

- [ ] 信号状态：进程处置/pending 与线程 blocked/pending、fork/exec 继承、自定义 handler 已接入；系统事件源待补
- [ ] 投递：syscall/时钟中断返回检查 pending，构造 Linux rt_sigframe；handler 经 restorer 调用 rt_sigreturn 已可用，wait4 已接入唤醒/EINTR，其他返回入口/阻塞唤醒待统一
- [ ] syscall：rt_sigaction/procmask/pending/return、gettid 与正 PID/TID 的 kill/tkill 已接入；进程组、停止/继续与实时信号待补
- [ ] 信号源：SIGPIPE、SIGCHLD、SIGINT（TTY VINTR→前台组）已接入；setitimer→SIGALRM 待补
- [ ] wait4 语义补全：已实现指定 PID / `pid=-1`、WNOHANG、正常/信号退出状态、一次回收、EFAULT 写回检查、信号唤醒/EINTR 及 SA_RESTART；孤儿收养和 rusage 待补
- [ ] 验证:busybox sh 交互及 ^C 中断前台忙循环/sleep/cat/管道、后台组隔离已通过；kill applet 完整场景待补

## 阶段 4:shell 体验完善 —— 完整 busybox sh + 大多数 applet

- [ ] `poll`(7)/`select`(23)——ash `read -t`、`timeout`
- [x] `ioctl`(16):TCGETS/TCSETS*/TIOCGWINSZ、控制会话与前台 PGID；基础 canonical/raw/ISIG 生效
- [ ] `getrlimit/setrlimit`(97/160,stub 合理默认)、`sysinfo`(99)、`times`(100)
- [ ] `getrandom`(318,musl getentropy 用,接现有 RNG)
- [ ] wall clock 完整:`date`、`ls -l` 时间戳、inode mtime
- [ ] 验证:busybox sh 有提示符、行编辑、`free`、`timeout`、`stty` 等可用

## 阶段 5:基础 pthread —— 常用多线程 musl 程序

- [ ] `clone`(56)+ `futex`(202):pthread 多线程
- [ ] TLS/TCB、`set_tid_address`、`clear_child_tid` 与 `exit_group` 的线程语义
- [ ] 验证:静态 musl pthread create/join、mutex/condvar、线程退出

## 阶段 6:扩展(非阻塞基础兼容主线)

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

## 差距状态表

| 差距 | 状态 | 后续入口 |
|---|---|---|
| Linux syscall 编号冲突、未知 syscall 杀进程 | 已修复 | 自动化兼容探针防回归 |
| 错误值不是 `-errno` | 已修复主路径 | 补全所有 syscall 的 errno 边界测试 |
| 自定义 getdents 结构不兼容 musl | 已修复 | ABI 布局探针 |
| execve 缺少 auxv、栈布局和 padzero 错误 | 已修复 | ELF/musl 启动探针 |
| wait4、WNOHANG、状态编码与回收 | 正常/信号退出子集已实现，rusage 待补 | `docs/tasks/07-process-signals-wait4.md` |
| 信号、SIGCHLD、SIGPIPE、SIGINT | 跨进程信号、wait4 EINTR/重启及 SIGPIPE 可用；SIGCHLD/前台 SIGINT 待补 | `docs/tasks/07-process-signals-wait4.md` |
| syscall 直接解引用用户指针 | 未修复，高风险 | `docs/tasks/05-user-memory-boundary.md` |
| fork/COW 有 shootdown，但无逐 CPU 完成确认 | 部分修复，高风险 | `docs/tasks/06-smp-memory-scheduler.md` |
| RTC 已读取，但 inode 时间戳和时间精度不完整 | 部分完成 | `docs/tasks/10-basic-musl-linux-runtime.md` |
| poll/select、rlimit、sysinfo、times、getrandom | 未实现 | `docs/tasks/10-basic-musl-linux-runtime.md` |
| clone/futex/pthread | 未实现 | `docs/tasks/11-pthread-clone-futex.md` |
| 进程组、作业控制、最小 procfs | 未实现 | `docs/tasks/12-procfs-job-control.md` |
| 动态链接器仅支持少量 reloc | 明确不纳入基础主线 | 保持静态链接，后续独立评估 |

## 2026-09-12 基础补齐与前端生命周期

- 修正 Linux chdir(80) 路径 ABI，新增 fchdir(81) 与 getcwd(79)；自定义用户库同步迁移。
- 修正 /mem 挂载方向与 shell cd 句柄泄漏，提供 HOME=/。
- 窗口句柄接入进程镜像资源所有权，增加退出/exec 清理、owner 校验、帧引用固定、
  创建失败回滚和脏区域传播；完整用户映射释放契约仍待专项补齐。
- 新增 test_frontend / test_shell_probe，交互 runner 扩至 8/16 核并验证命令实际新输出。
- 用户当前实机（约 8～16 核）sh 命令无明显输出尚待新镜像复测，不能用 QEMU PASS 替代。
- 实现、边界与后续路线：[本批记录](docs/learning/frontend-lifecycle-cwd.md)。

## 2026-09-13：终端前端基础

- 独立字符模型与跨 write UTF-8/ANSI；基础颜色、光标/擦除/滚屏、真实 winsize。
- TTY OPOST/ONLCR 字节输出和串口一致；终端模型接入窗口背景恢复。
- 1/4/16 核交互与真实像素/字节验证通过，支持子集及剩余任务见
  [本批交接](docs/SESSION-HANDOFF-2026-09-13.md)。
- 仍不声明完整 VT/PTY 或原实机问题已解决。
