# wait4 的 SA_RESTART：从信号帧回到调用指令

## 问题与行为

上一批 wait4 被可处理信号打断后统一返回 EINTR；SA_RESTART 虽然能注册，还没有效果。
本批让调用者得到两种明确行为：

| handler 标志 | handler 返回后 |
| --- | --- |
| 不设置 SA_RESTART | 原 wait4 返回 -EINTR，调用者决定是否重试 |
| 设置 SA_RESTART | 重新执行原 wait4，继续等待并返回子 PID/退出状态 |

两种情况都先运行 handler。被打断的那次等待不会写 status，也不会提前回收子进程。

## 核心方法

[syscall](../../mos_syscall/src/lib.rs) 在分发前记住原系统调用号，分发后只有同时满足
“原调用是 wait4”和“返回 -EINTR”才传递重启资格。其他错误、成功结果以及
rt_sigreturn 恢复出的任意 RAX 都不会被误判为可重启调用。

[deliver_syscall_pending](../../mos_syscall/src/signal.rs) 在选定实际投递的 handler 后，
使用该 handler 的 SA_RESTART 标志决定是否修改将要保存的现场：

```rust
// wait4 保持原参数寄存器；这里只恢复调用号与指令位置。
context.scratch.rax = 61;
context.iret.rip -= 2;
```

这是概念片段；实际 `restart_syscall` 使用 checked_sub 检查减法边界。
目前两个入口的调用指令分别为 `0F 05`（syscall）和 `CD 80`（int 0x80），均为两字节，
硬件保存的返回 RIP 位于指令后。RDI/RSI/RDX/R10 等原参数保持不变。

修改发生在 build_frame 之前：恢复后的 RAX/RIP 被写进用户 ucontext，之后内核才将
执行入口指向 handler。handler 的 RET → restorer → rt_sigreturn 恢复该现场，
CPU 再执行调用指令。此时 wait4 重新扫描子进程，因而也能看到 handler 运行期间发生的退出。

重启资格是本次 Rust 调用的局部数据，没有保存在 TCB 中。handler 中调用 wait4、
raise 或产生嵌套信号，不会消耗外层的重启记录。用户仍能修改 ucontext：测试会主动
把 RIP 调回调用指令之后并设置 RAX=123，确认 sigreturn 尊重修改，不强制再次等待。

## 回归揭示的入口修复

初次 QEMU 回归中 syscall 用例通过，但 int 0x80 在进入内核处理函数之前触发 #GP，
错误码为 1026。检查发现该 IDT 项误用了普通设备中断注册，DPL 保持 ring 0。

[idt 初始化](../../mos_runtime/src/idt.rs) 改用已有的 set_trap_handler，允许 ring 3
调用这个软件中断；设备中断配置保持原有权限。没有新增汇编返回实现。

这里测试的是 MOS 原有的 x86_64 参数/调用号约定，并不表示实现了 Linux int 0x80
的 i386 兼容 ABI；32 位程序加载和兼容调用表仍不在本批范围。

## 测试

[test_signal_restart](../../user/user_space/src/bin/test_signal_restart.rs) 包含 6 个组合：

- syscall 与 int 0x80 分别验证无 SA_RESTART 时得到 EINTR。
- 两个入口分别验证有 SA_RESTART 时自动等待并返回正确 PID/退出码。
- 两个入口分别验证 handler 编辑 ucontext 可以取消已经准备好的重启。

handler 检查保存的 RAX、RIP 处真实指令字节、未修改的 status，还执行一次返回
ECHILD 的 wait4 和嵌套 SIGUSR2。子进程必须收到 handler 的管道确认后才能退出，
避免单纯的子退出成功被当作重启测试通过；开始发送前的短暂延迟用于让父进程进入等待。
最后再次 wait4 必须返回 ECHILD，确认没有重复回收。

运行完整镜像后，使用 `tools/qemu-wait4.ps1 -Cpus 1` 和
`tools/qemu-wait4.ps1 -Cpus 4 -MonitorPort 55564`。

2026-09-10 最终验证：全量用户程序和完整内核 release 镜像构建成功；最新镜像在
单核/4 核 QEMU 中均通过 wait4、process baseline、signal state/exec/delivery/cross/
restart 及 BusyBox sh/echo。日志为 `build/wait4-qemu-1.log` 和
`build/wait4-qemu-4.log`。已有编译 warning 未在本批清理。

## 后续边界

本批只为 wait4 开放重启资格。read、pipe、nanosleep 等其他阻塞接口尚未接入统一
信号唤醒协议，不能据此声称整个系统的 SA_RESTART 已完成。下一批优先建立可取消的
等待注册，处理事件完成与信号唤醒竞争，再按每个 syscall 的语义选择 EINTR 或重启。

后续更新：管道 read/write 和字节 I/O 的重启现已接入，见[可中断管道](interruptible-pipe.md)；
TTY、nanosleep 和其他等待仍待推进。
