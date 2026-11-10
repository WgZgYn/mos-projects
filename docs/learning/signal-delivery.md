# 从 pending 到 handler 再返回：信号投递第一版

对应任务 07 阶段 B。上一批保存了信号状态；这一批让自定义 handler 真正执行并返回。
目前信号源限于单线程进程向自身发送普通信号；完整跨进程发送与可中断等待不在本批。

## 问题、方法、结果

| 问题 | 方法 | 用户可见结果 |
|---|---|---|
| syscall 返回占用 RCX/R11 | 保存独立 IRET 帧，改用 iretq | sigreturn 可独立恢复 RCX、R11、RIP、RFLAGS |
| handler 会覆盖寄存器、栈、浮点状态 | Linux rt_sigframe + 512 字节 FXSAVE 状态 | handler 完成后原程序继续执行 |
| 用户可以篡改返回栈帧 | 检查地址、段选择子、MXCSR，过滤特权标志 | 坏帧转为用户 SIGSEGV 退出，不在内核 IRET/FXRSTOR 时异常 |
| 同一信号可能重入或重复发送 | blocked/pending、NODEFER、RESETHAND | 支持默认屏蔽、标准信号合并、嵌套及一次性处置 |
| wait 无法区分 exit(12) 与被信号 12 杀死 | SharedState 保存已编码 wait_status | 正常退出 0x0c00，信号终止 0x000c |

## 一条完整路径

```text
用户 kill(自身 PID, SIGUSR1) / raise(SIGUSR1)
  → syscall 将信号记入 pending
  → syscall 返回用户态前检查 pending & !blocked
  → 写入用户栈上的 rt_sigframe 与 FXSAVE 状态
  → IRET 到 handler(signum, siginfo, ucontext)
  → handler 的 ret 跳到 libc/用户库 restorer
  → restorer 调用 syscall 15（rt_sigreturn）
  → 检查并恢复寄存器、浮点状态、屏蔽掩码
  → IRET 回被中断位置
```

时钟中断的用户返回也接入同一投递函数。来自内核态的中断不投递，防止把用户 handler
装到内核执行现场。当前测试通过自身 syscall 触发投递，尚未证明跨 CPU 异步发送。

## 为什么改变 syscall 返回汇编

[register_syscall](../../mos_arch/src/asm.rs) 原来只保存 user_rsp，利用 RCX/R11 的特殊
硬件用途执行 SYSRET。sigreturn 的 ucontext 却可以分别指定 RCX、R11 与 RIP、RFLAGS。
因此现在 syscall 入口构造与硬件中断一致的尾部：

```text
RIP → CS → RFLAGS → RSP → SS
```

前面仍是保存的通用寄存器，总计 160 字节，由编译期断言保护布局。
退出时先关中断，再恢复寄存器和 IRET，避免在半恢复状态继续执行可重入内核代码。
fork/exec 和 int 0x80 分发同步使用这一完整上下文。

## Linux 栈帧布局与对齐

[signal_frame.rs](../../mos_abi/src/signal_frame.rs) 固定以下 x86_64 内核 ABI 偏移：

| 从 frame base 起算 | 内容 |
|---|---|
| 0 | restorer 返回地址（pretcode） |
| 8 | ucontext |
| 48 | sigcontext：寄存器现场 |
| 304 | 64 位信号掩码 |
| 312 | 128 字节 siginfo |
| 440 之后、独立对齐位置 | 512 字节 FXSAVE 图像 |

栈帧避开原 RSP 以下 128 字节 red zone；handler 入口满足 RSP % 16 == 8。
FPU 区域按 16 字节对齐。所有内容通过受检 copy_to_user 写回，避免未映射/COW 页问题。
handler 返回时已经弹出了 pretcode，因此 rt_sigreturn 从 `RSP - 8` 找 frame base。

本版使用 legacy FXSAVE，不声明 XSAVE/AVX 扩展。不支持 alternate signal stack。
ABI 对照：[Linux rt_sigframe](https://github.com/torvalds/linux/blob/master/arch/x86/include/asm/sigframe.h)、
[sigcontext 与 fpstate](https://github.com/torvalds/linux/blob/master/arch/x86/include/uapi/asm/sigcontext.h)。

## 投递与恢复的核心约束

[deliver_pending](../../mos_syscall/src/signal.rs) 先选择最低编号的可投递信号，再保存旧掩码。
handler 运行期间屏蔽 `旧掩码 | action.mask | 当前信号`；有 SA_NODEFER 时不自动添加当前信号。
SA_RESETHAND 在进入 handler 前把处置改回默认。标准信号重复发送只保留一个 pending 位。

[restore_frame](../../mos_syscall/src/signal.rs) 不把用户栈当可信上下文直接加载：

- 返回 RIP 必须在用户可执行区域，RSP 必须是低半区地址。
- CS/SS 必须是 MOS 用户段选择子，不能通过帧切换到 ring 0。
- RFLAGS 只恢复当前支持的用户标志，IF 和保留位 1 强制置位；IOPL/NT/VM 等不能注入。
- FPU 地址须可读取并对齐；MXCSR 用硬件支持掩码检查，拒绝不支持的 xstate 扩展。
- 所有检查通过后才提交上下文；失败按 SIGSEGV 终止此进程。

当前 flags 恢复子集不含单步调试 TF 等控制状态。投递初版仅保存 SA_RESTART；后续
已接入 wait4 的 EINTR 和重启，见[重启学习笔记](wait4-restart.md)。其他阻塞接口仍待接入。

## 结果如何测试

[test_signal_delivery](../../user/user_space/src/bin/test_signal_delivery.rs) 覆盖：

1. 真实 handler 调用、SA_SIGINFO 的 signo/code、handler 内的掩码。
2. 修改 ucontext 中 RAX/RCX/R11 后恢复；其中 RCX/R11 故意与 RIP/RFLAGS 不同。
3. handler 清零 XMM15 后，返回恢复原 SIMD 数据；red zone 内容和通用寄存器不变。
4. 屏蔽期间 pending 查询与合并、解除屏蔽投递、SIG_IGN 清除 pending。
5. 默认非重入与 SA_NODEFER 嵌套，SA_RESETHAND 一次性处置。
6. 坏 RIP、CS/SS、MXCSR、fpstate 地址、RSP、伪造 sigreturn 及不可用投递栈。
7. 父进程 wait4 获得 SIGSEGV／SIGUSR2 终止状态，且自身仍能继续运行。

`test_signal_state` 同步验证 fork 继承真实自定义 handler、exec 清除 handler，并保留
忽略处置及掩码。现有 wait4、stderr、BusyBox sh/echo 场景一起回归。

构建当前完整镜像后运行：

```powershell
./tools/qemu-wait4.ps1 -Cpus 1
./tools/qemu-wait4.ps1 -Cpus 4 -MonitorPort 55564
```

2026-09-10 最终验证：全量用户程序和完整内核镜像 release 构建成功，`mos_abi`
的 4 个宿主测试通过；最新镜像在单核和 4 核 QEMU 中均通过 wait4、process baseline、
signal state/exec/delivery、BusyBox sh/echo。串口记录分别在
`build/wait4-qemu-1.log` 和 `build/wait4-qemu-4.log`。已有编译 warning 未在本批清理。

## 下一批的边界

2026-09-10 更新：正 PID/TID 的跨进程发送、发送者信息和 wait4 的唤醒/EINTR 已实现，
详见[跨进程信号学习笔记](cross-process-signals.md)。以下列表和 self-only 限制记录本批当时边界。

- 信号源：跨进程 kill/tkill、子退出 SIGCHLD、管道 SIGPIPE、终端 SIGINT。
- 等待：blocked 任务如何被信号唤醒、EINTR 与 SA_RESTART 的返回点。
- 生命周期：SIGCHLD 忽略/NOCLDWAIT 自动回收、进程组、停止/继续、多线程退出。
- 内存：非驻留文件页的安全 fault-in、COW 与跨 CPU TLB 完成确认。
- 浮点/调试扩展：XSAVE/AVX、alternate stack、更多可恢复调试标志。

当前 self-directed kill/tkill 遇到其他目标、多线程进程、停止/继续信号或实时信号返回 ENOSYS，
未给这些路径返回伪成功。用户态测试采用 Rust 原始 Linux ABI，BusyBox 为 musl 静态程序；
尚未单独运行 musl C raise/sigaction 探针。
