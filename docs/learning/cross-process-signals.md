# 跨进程信号与 wait4 可中断等待

本批在已有 handler/rt_sigreturn 上补充正 PID/TID 寻址、真实发送者信息，以及
wait4 的 EINTR 返回。沿用完整 IRET 上下文，无需新增汇编返回路径。

## 问题：发送目标可能正在退出

[find_user_target](../../mos_task/src/thread.rs) 在 TCB slab 锁内查找用户任务，
按 PID 或 TID 比较，过滤已经发布退出状态的任务。返回前必须取得一个强引用。

不能简单用裸指针构造 KRef，也不能无条件增加引用计数：最后一个引用可能已经
减到零，而销毁函数还在等待 slab 锁。这里用非零计数的原子条件增加，拒绝复活
正在销毁的对象。释放 slab 锁后，发送方持有的 KRef 保证目标存储仍有效。

当前查找遍历 slab 已分配槽位；先保证生命周期正确，后续有规模需求时再引入索引。
TID 仍沿用 slab 槽位，并不保证主线程 TID 等于 PID，这是已有的 Linux 兼容债务。

## 方法：发布 pending 与进入等待使用同一把锁

[sys_send_signal](../../mos_syscall/src/signal.rs) 的关键次序是：

```text
发送方：取得目标 child_exit_queue 锁
        → 写 pending、发送者 PID 和 si_code
        → 若可投递且非忽略处置，取出 wait4 等待者
        → 释放信号锁和队列锁
        → 通过调度器唤醒

等待方：取得自己的 child_exit_queue 锁
        → 优先检查子进程退出状态、ECHILD、WNOHANG
        → 检查可打断等待的 pending
        → 有信号则返回 EINTR；否则登记等待者
        → 调度器设置 Blocked 后释放队列锁
```

这保证发送方不能插入“等待方最后一次检查完、还没有登记等待”的空隙。
发送方释放队列锁后才进入调度器，避免与持有队列锁进入 park 的等待方锁序冲突。
信号唤醒会取走等待队列条目，不留下之后误唤醒同一任务的旧条目。

[interrupts_wait](../../mos_syscall/src/signal.rs) 过滤屏蔽和忽略的信号。
被中断的 wait4 返回 `-4`（EINTR），不写 status、不回收子进程；返回用户态前
通过已有 deliver_pending 进入 handler，handler 返回后用户程序可以重新 wait4。
若扫描时已有可回收的子进程，则先返回该子进程结果，信号仍由返回路径处理。

## 结果：发送者信息跨进程保留

每个标准 pending 位旁保存最早一次发送的 PID 和 code；重复发送合并，不覆盖来源。
kill 使用 SI_USER（0），tkill 使用 SI_TKILL（-6）。构造用户 siginfo 时读取这些字段，
不再把接收进程自身 PID 当作发送者。当前所有用户进程 UID 都是 0，si_uid 也为 0。

正 PID/TID 不存在时返回 ESRCH；signal=0 只探测存活目标。进程组发送仍返回
ENOSYS，tkill 的非正 TID 返回 EINVAL。实际信号发送仍限单线程用户进程和普通信号，
多线程目标、停止/继续及实时信号维持 ENOSYS。已经退出的目标按 ESRCH 处理；
尚未提供 Linux zombie 生命周期中的完整 kill 探测语义。

## 测试入口

[test_signal_cross](../../user/user_space/src/bin/test_signal_cross.rs) 覆盖：

- 父进程按 PID/TID 向子进程发送；子进程纯用户态循环验证时钟返回投递。
- siginfo 的真实发送者 PID，以及 SI_USER/SI_TKILL 来源区别。
- 不存在的目标、非法参数、signal=0 和回收后的 PID 查询。
- 8 轮即时/延迟发送覆盖 wait4 检查与 park 附近的时序；EINTR 不修改 status，
  重试能够回收正确退出码。此回归增加竞态暴露机会，不代替锁序正确性分析。
- 屏蔽期间合并 pending、不打断 wait4，解除屏蔽后投递，发送者退出后信息仍有效。
- SIG_IGN 不打断等待；跨进程 SIGKILL 产生正确 wait4 信号退出状态。

运行完整镜像后，使用 `tools/qemu-wait4.ps1 -Cpus 1`，以及
`tools/qemu-wait4.ps1 -Cpus 4 -MonitorPort 55564`。

2026-09-10 验证结果：内核检查、完整 release 镜像和全量用户程序构建成功，4 个
mos_abi 宿主测试通过。单核/4 核 QEMU 均通过 wait4、process baseline、signal
state/exec/delivery/cross 及 BusyBox sh/echo。日志在 `build/wait4-qemu-1.log` 和
`build/wait4-qemu-4.log`；本批没有清理既有编译 warning。

## 下一批

当前只有 wait4 接入可中断等待。read、pipe、nanosleep 等阻塞接口仍可能延迟处理
pending，不能认为发送成功就保证立即终止所有阻塞任务。本批最初只返回 EINTR；
后续已补 wait4 的 SA_RESTART，见[重启学习笔记](wait4-restart.md)。

接下来优先补通用可中断等待注册及各阻塞 syscall 的重启语义，再接 SIGCHLD/SIGPIPE/
终端 SIGINT。内核产生 SIGCHLD 时还需保存退出者状态等独立元数据，不能直接复用
当前用户发送的 SI_USER 路径。多线程投递、exit_group 和 UID 权限模型后续一起完善。
