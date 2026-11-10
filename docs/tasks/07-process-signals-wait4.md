# 任务 07：wait4、信号与进程退出语义

**类别：Linux 兼容功能开发 / 内核开发编码**

## 可直接交给新对话的任务指令

实现满足静态 musl/BusyBox 交互 shell 所需的最小 Linux 进程等待和信号子系统。严格遵循 Linux x86_64 syscall 号、返回值、状态编码和用户态 ABI。分阶段提交：先 wait4/僵尸模型，再信号状态与投递，再终端 Ctrl-C 和 SIGPIPE。

## 背景

### 2026-09-11：真实 BusyBox 前台 Ctrl-C

- 控制终端和前台 PGID ioctl 已接入，输入行规程按 ISIG/VINTR 向前台组发送 SIGINT。
- BusyBox ash 作业控制实际启用；忙循环、sleep、cat、管道和空提示符恢复均通过，后台组隔离。
- wait4 接受 ash 使用的 WUNTRACED，但停止/继续状态仍未实现。
- 单核/4 核专项与完整回归通过；说明：[控制终端与前台 Ctrl-C](../learning/tty-foreground-ctrl-c.md)。

### 2026-09-10：SIGCHLD 退出通知

- 正常/信号退出发布 SIGCHLD，携带子 PID、CLD_EXITED/CLD_KILLED 和状态。
- 退出记录与通知在父等待队列/信号锁内发布，再唤醒 wait4 和可中断等待。
- 新增 test_sigchld；学习说明：[SIGCHLD 与 wait4 回收](../learning/sigchld.md)。
- 验证：kernel check、用户程序/完整镜像 release 构建、单核/4 核 QEMU 专项及完整回归通过。
- 显式忽略的自动回收、停止/继续、孤儿收养、多线程退出及 CPU 时间记账仍待补。

### 2026-09-10：nanosleep 可中断等待

- nanosleep 改为受检输入/剩余时间回写，支持 EINTR、别名与 EFAULT；不自动 SA_RESTART。
- 定时事件使用每次等待独立 waker，退出等待时取消登记，已取出的旧事件由 token 失效保护。
- 新增 test_sleep_interrupt，覆盖错误输入、中断、后续等待和 SIGKILL。
- 验证通过：kernel check、用户程序/完整镜像 release 构建、单核/4 核 QEMU 新增测试和既有回归。
- 学习说明：[可取消的定时等待](../learning/interruptible-sleep.md)。

### 2026-09-10：管道可中断等待

- 新增每次 park 独立的 InterruptWait，调度器内确认 token 仍有效再唤醒；恢复后失效并清理队列项。
- 管道 read/read_to/write 接入信号唤醒和 EINTR，字节 I/O 返回路径接入 SA_RESTART。
- 新增 test_pipe_interrupt，handler 确认门控读写/重启场景，以及阻塞 SIGKILL。
- 验证：用户程序与完整镜像 release 构建、kernel check、单核/4 核 QEMU 新增测试及完整旧回归通过。
- TTY/计时/FIFO open 仍需各自迁移。学习说明：[可中断管道](../learning/interruptible-pipe.md)。

代码学习入口：[进程等待与信号状态：代码阅读及维护笔记](../learning/process-wait-signals.md)。

### 2026-09-10 编码进展（阶段 C：SIGPIPE 与真实 shell 管道）

- write/writev 共享 BrokenPipe → 当前线程 SIGPIPE；默认/忽略/handler/屏蔽处置复用现有投递。
- 零长管道操作不阻塞、不触发 SIGPIPE；writev 后续失败保留已经写入的字节数。
- 真实 BusyBox yes/head 管道揭示 FS base 切换错误：移入 __switch_to，涵盖 fork 首次返回，
  去掉使用旧栈 new_ctx 的错误恢复；ARCH_SET_FS 的硬件/保存值更新防止中间抢占。
- 新增 test_sigpipe 和 test_fork_tls。学习说明：[SIGPIPE 与 shell 管道](../learning/sigpipe-and-shell.md)。
- Ctrl-C 尚未完成：可中断 I/O、进程组和 TTY 输入处理按 [任务 14](14-tty-terminal.md) 接续。
- 最终验证：完整 release 构建、4 个 ABI 宿主测试，以及单核/4 核 QEMU 的全部旧回归、
  TLS 专项、SIGPIPE 和真实 BusyBox yes/head 管道均通过。

### 2026-09-10 编码进展（阶段 B：wait4 SA_RESTART）

- wait4 返回 EINTR 后，根据实际投递 handler 的 SA_RESTART 设置准备重启的 RAX/RIP；
  重启现场保存在信号帧中，handler 返回后重新执行调用指令。其他 syscall 未开放重启。
- 重启资格只在本次分发/投递间传递，不污染 handler 内 syscall 或嵌套信号；
  用户编辑 ucontext 可以主动取消重启。
- 回归发现 int 0x80 门误设为内核权限，现改用已有的 ring 3 trap 注册接口。
  该入口沿用 MOS x86_64 参数约定，不代表支持 Linux i386 兼容 ABI。
- 新增 test_signal_restart，覆盖两个入口的 EINTR/自动重启/ucontext 编辑共 6 个组合，
  同时验证嵌套信号、handler 内 wait4、status 不提前写回与一次回收。
- 学习入口：[wait4 的 SA_RESTART](../learning/wait4-restart.md)。
- 验证：全量用户程序、完整内核 release 镜像构建成功，单核/4 核 QEMU 的
  wait4、process baseline、signal state/exec/delivery/cross/restart 和 BusyBox sh/echo 全部通过。

### 2026-09-10 编码进展（阶段 B：跨进程发送与 wait4 EINTR）

- 正 PID/TID 的 kill/tkill 已接入存活用户目标；slab 锁内取得非零强引用，避免查找与退出竞态。
- pending 保存首个发送者 PID/code，跨进程 handler 得到正确 SI_USER/SI_TKILL 信息。
- wait4 与信号发送共用 child_exit_queue 同步检查/登记，接入唤醒和 EINTR；
  优先返回已有子退出结果，EINTR 不修改 status、不消耗子记录。
- 新增 test_signal_cross：用户态循环异步投递、PID/TID、来源信息、signal=0/ESRCH、
  8 轮等待时序、屏蔽合并、忽略信号和 SIGKILL；完整 QEMU runner 已纳入。
- 仍限单线程用户进程；进程组、停止/继续、实时信号和完整 zombie 探测语义待补。
  其他阻塞接口与 SA_RESTART 尚未接入，当前 wait4 对可处理信号返回 EINTR。
- 核心代码与锁序解释：[跨进程信号与 wait4 可中断等待](../learning/cross-process-signals.md)。
- 验证：完整 release 镜像及全量用户程序构建成功，4 个 ABI 宿主测试通过；
  单核/4 核 QEMU 的 wait4、process baseline、signal state/exec/delivery/cross、
  BusyBox sh/echo 全部通过。

### 2026-09-09 编码进展（阶段 B：handler 投递与 rt_sigreturn）

本批说明与核心代码：[从 pending 到 handler 再返回](../learning/signal-delivery.md)。

- syscall 入口改为完整 IRET 上下文，返回 RIP/RFLAGS 与 RCX/R11 分离；
  fork/exec/int 0x80 同步适配，退出恢复前关闭中断。
- 加入 Linux x86_64 rt_sigframe 布局，保存通用寄存器、掩码、siginfo、legacy
  FXSAVE 状态；跳过 red zone，满足 handler 栈与 FPU 区域对齐要求。
- 自定义 handler 已开放，要求 SA_RESTORER 及有效用户可执行 handler/restorer。
  支持 SIGINFO、NODEFER、RESETHAND、默认屏蔽与标准 pending 合并。
- `rt_sigreturn(15)` 先检查用户内存、段选择子、返回地址和 MXCSR，再恢复现场；
  坏帧或无法构造的投递栈按 SIGSEGV 退出，wait4 返回信号终止状态。
- 接入 `gettid(186)`、单线程进程 self-directed `kill(62)`/`tkill(200)`；其他目标、停止/继续、
  实时信号返回 ENOSYS。SIGCHLD/SIGPIPE/SIGINT 的系统事件源仍待实现。
- syscall 与时钟中断的用户返回路径接入投递，内核态中断不投递；其他中断返回入口
  后续随新信号源统一。SA_RESTART 暂存，阻塞 syscall 的 EINTR/重启尚未实现。
- `test_signal_delivery` 验证寄存器/ucontext 编辑、SIMD、red zone、嵌套、pending、
  一次性处置、坏帧及默认信号终止；state 测试升级为 fork/exec 的真实 handler 继承。
- 2026-09-10 最终验证：全量用户程序及完整内核镜像 release 构建成功，4 个 ABI
  宿主测试通过；最新代码的单核/4 核 QEMU 均通过 wait4、process baseline、
  signal state/exec/delivery 和 BusyBox sh/echo。

以下“控制接口第一步”是上一批历史记录；其“自定义 handler 不支持”的限制已由本批取代。

### 2026-09-09 编码进展（阶段 B：信号控制接口第一步）

- 添加 x86_64 内核 signal ABI：64 位 signal set、32 字节 KernelSigAction，
  布局和 mask 运算由 mos_abi 宿主测试覆盖。
- PCB 保存共享处置表及进程 pending；TCB 保存线程 blocked/pending。
  fork 复制处置和调用线程 mask、pending 清空；exec 保留 mask 和 SIG_IGN。
- 接入 `rt_sigprocmask(14)`：BLOCK/UNBLOCK/SETMASK，清除 KILL/STOP 位，
  NULL set 时忽略 how；输入与 oldset 重叠也可正确工作。
- 接入 `rt_sigaction(13)` 的查询与 DFL/IGN 存储子集；custom handler、
  SIGCHLD 显式忽略和 SA_NOCLDWAIT 返回 ENOSYS，避免承诺尚未实现的投递／自动回收。
- 接入 `rt_sigpending(127)` 查询；当前无信号源，不把返回空集合视为投递测试通过。
- 信号参数通过 `copy_from_user` 读取：VMA 权限/地址边界检查、跨页和匿名零页；
  文件页的非驻留 fault-in 仍待补。写回失败不提交新状态是当前 MOS 子集约定。
- 新增 `user_lib::signal` 封装及 `test_signal_state`：掩码操作、别名指针、无效
  指针/尺寸、跨页权限、拒绝自定义 handler、fork 独立副本、exec 继承。
- 回归入口沿用 `tools/qemu-wait4.ps1`，已纳入信号状态及 exec 测试。
- 验证结果：内核及全量用户态 release 构建成功，mos_abi 的 3 个宿主测试通过；
  单核/4 核 QEMU 均通过 wait4、process baseline、signal-state/exec 和 BusyBox sh/echo。
  构建仍有原有 unused 等 warning，本批未进行无关清理。

当前下一批：其他 blocked 任务唤醒及各 syscall 重启语义、SIGCHLD/SIGPIPE/SIGINT；
仍保留 SIGCHLD 显式忽略/NOCLDWAIT、altstack、XSAVE 等限制。

### 2026-09-09 编码进展（阶段 A 最小 wait4）

- 初始进程补齐 fd 2，与 stdout 共享输出文件，fork 继续继承。
- 退出码在 `exited` 的 Release 发布前写入，避免等待方读到默认退出码。
- fork 内存准备成功后才登记子进程；失败路径同样执行现有 TLB shootdown。
- 新增 `test_process_baseline`，覆盖 stderr、描述符占用、64 次 fork/退出码和重复回收。
- syscall 61 改为返回子 PID，向 status 写入 `(code & 255) << 8`；支持正 PID、
  `-1`、WNOHANG、ECHILD、NULL status。旧 user_lib::waitpid 继续解码返回退出码。
- 父进程统一退出等待队列，扫描至注册等待持有同一队列锁，避免漏唤醒；
  子记录由 children 锁保护，写回成功后才回收，EFAULT 后可重试。
- PID 与 PCB slab 槽位解耦，单调分配且不回绕；耗尽返回 EAGAIN。
- 新增受 VMA 锁保护的物理映射写回，检查低半区地址、溢出、写权限与范围，
  支持匿名按需分配、COW 和跨页 status；不直接解引用用户地址。
- 内核、全量用户程序 release 构建及 ABI 宿主测试通过；QEMU 单核/4 核均通过
  `test_wait4`、`test_process_baseline` 和静态 musl BusyBox echo。
- BusyBox `sh -c /test_exit` 揭示 exec 后残留旧 `clear_child_tid` 导致退出写零崩溃；
  已在 exec 成功加载后清除此地址，退出写零也改用受检写回并忽略 EFAULT。
  回归脚本增加该 BusyBox 场景，`test_wait4` 增加非法退出通知地址的测试；
  修复后所有场景在单核/4 核均通过。

阶段 A 保持部分完成：信号终止状态已接入；孤儿收养和完整多线程退出生命周期尚未实现。

当前明确限制：

- 进程组选择（pid=0 / pid<-1）、非 NULL rusage 返回 ENOSYS；
- WUNTRACED/WCONTINUED 等尚未支持的 options 返回 EINVAL；
- 写回已驻留普通文件页可用，未驻留文件页和设备映射目前返回 EFAULT；
- COW 仍使用现有无逐 CPU ack 的 shootdown，4 核回归通过不等于已解决任务 06；
- 非法 status 不消耗子记录是当前最小子集的约定，后续与 Linux 完整错误路径对照。

回归入口：构建最新用户态/ramfs/内核镜像后，运行
`tools/qemu-wait4.ps1 -Cpus 1` 和 `tools/qemu-wait4.ps1 -Cpus 4`。
串口日志位于 `build/wait4-qemu-{1,4}.log`。测试包含 pipe 门控的 WNOHANG、
多子进程、退出码截断、重复回收、非法/内核地址、跨页权限和存活子进程的 COW 隔离。
测试源：`user/user_space/src/bin/test_wait4.rs`。
ABI 对照：[Linux waitpid](https://man7.org/linux/man-pages/man2/waitpid.2.html)、
[Linux wait4](https://man7.org/linux/man-pages/man2/wait4.2.html)。

此外：

- `exit_group` 仅调用 `sys_exit`，不会结束同进程其他线程；
- `clear_child_tid` 不做 futex wake；
- `rt_sigaction`/`rt_sigprocmask`/`rt_sigpending`、handler/rt_sigreturn 和自身发送已接入；
  跨进程发送与可中断等待仍待实现；
- 没有 SIGCHLD、SIGPIPE、SIGINT 和用户返回路径信号投递；
- init 初始文件表已补齐 fd 2。

## 分阶段范围

### 阶段 A：wait4 与进程生命周期

1. 实现 Linux `wait4(pid, status, options, rusage)` 最小子集。
2. 支持指定 PID、`pid=-1`、`WNOHANG`、无子进程 `ECHILD`。
3. 正确编码正常退出和信号终止状态。
4. 明确 running → zombie → reaped 生命周期，避免 PID/资源过早释放。

### 阶段 B：信号状态和 ABI

1. 为进程/线程加入 pending、blocked mask、action 表。
2. fork 继承 action/mask；exec 清除应清除的 handler 并保留 mask。
3. 实现 Linux x86_64 `rt_sigframe` 和 `rt_sigreturn`。
4. 在 syscall/interrupt 返回用户态的统一路径投递信号。

### 阶段 C：信号源和 shell 行为

1. 子进程退出产生 SIGCHLD。
2. 写入无 reader 的 pipe 产生 SIGPIPE，并返回 EPIPE。
3. tty Ctrl-C 向前台进程/进程组产生 SIGINT。
4. 实现 `kill`、`tkill`；如作业控制尚未实现，明确最小前台模型。
5. 补齐 init 的 stdin/stdout/stderr。
6. `exit_group` 正确结束同进程所有线程。

## 主要文件

- `mos_abi/src/syscall.rs`
- `mos_abi/src/error.rs`
- 可新增 `mos_abi` 信号 ABI 定义
- `mos_syscall/src/lib.rs`
- `mos_syscall/src/process.rs`
- 可新增 `mos_syscall/src/signal.rs`
- `mos_task/src/process.rs`
- `mos_task/src/thread.rs`
- `mos_task/src/scheduler.rs`
- `mos_runtime/src/idt.rs`
- `mos_arch` 的用户返回汇编路径
- `mos_fs_runtime/src/pipe.rs`
- `mos_devfs/src/tty.rs`
- `mos/src/main.rs`
- `user/user_lib/src/syscall.rs`

## 非目标

- 第一版不要求完整 POSIX job control、ptrace、实时信号队列或完整 rusage。
- 不改变 MOS 私有 syscall 编号。
- 不使用与 Linux 不兼容的自定义 sigframe。

## 验收标准

- `wait4(-1, ..., WNOHANG, ...)` 行为正确。
- 父进程只能回收一次子进程；僵尸在 wait 前保留必要状态。
- BusyBox `sh` 可启动子命令并获得正确退出状态。
- `kill` applet 可发送支持的信号。
- Ctrl-C 能中断前台任务而不杀死 shell 或内核。
- 关闭 reader 后写 pipe 产生 SIGPIPE/EPIPE。
- 多线程程序调用 `exit_group` 后不残留线程。

## 验证方式

新增自动用户态测试：

1. 多个子进程以不同 exit code 退出，分别用指定 PID 和 `-1` 等待；
2. WNOHANG 在运行中返回 0，退出后返回 PID；
3. SIGCHLD handler/default/mask；
4. 自定义 handler 返回后寄存器和栈恢复；
5. SIGPIPE 与忽略 SIGPIPE；
6. Ctrl-C 前台交互；
7. exit_group 多线程退出。

最后运行 BusyBox shell、`kill`、管道和前台中断 smoke。
