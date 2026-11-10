# SIGCHLD：退出通知与 wait4 回收

## 问题

子进程原来只唤醒父进程的 wait4 队列。父进程运行用户代码或阻塞在其他接口时，
无法通过 SIGCHLD 得知子进程退出。单独在退出末尾发送信号还会产生发布顺序问题：
父进程可能先处理通知，再检查到尚未完成的退出记录。

## 方法与代码入口

`mos_syscall/src/process.rs::exit_with_status` 调用
`mos_syscall/src/signal.rs::notify_child_exit`。此函数先按稳定 PID 取得存活父线程，
再锁住父 wait4 队列与信号状态。它写入 wait status、SIGCHLD 元数据和 pending，
在释放信号锁前用 Release 发布 exited。wait4 的 Acquire 读取和 handler 的信号锁
分别保证两条观察路径都能看到完整状态。所有调度唤醒都发生在资源锁释放之后。

父进程可以处于 wait4、可中断管道、nanosleep 或普通用户态执行中。前两类等待和
定时等待沿用已经实现的唤醒协议；普通用户态在定时中断返回时处理 pending。
不在子进程中长期持有父线程强引用，避免父进程先退出时形成生命周期依赖。

`ThreadSignals::pending_status` 补充每个标准信号的来源状态。
SIGCHLD 的 si_code 为 CLD_EXITED（1）或 CLD_KILLED（2），si_pid 为子 PID，
si_status 为退出码或终止信号号。信号帧在 x86_64 siginfo 的偏移 24 写入状态；
普通 kill/tkill 仍保存自己的来源信息，并清零该状态，防止复用旧数据。
参见 [sigaction 的 SIGCHLD 字段说明](https://www.man7.org/linux/man-pages/man2/rt_sigaction.2.html)。

标准信号合并时保留首个来源；每个子进程的 wait4 记录独立存在。因此 handler 应循环
调用 wait4(WNOHANG)，不能以信号次数代替退出子进程数。本次不自动回收子进程。

## 测试

`test_sigchld` 覆盖正常退出中断父 nanosleep、handler 内立即 WNOHANG 回收、
正常退出及 SIGKILL 的来源/原因/状态、重复回收 ECHILD、屏蔽时两个退出通知合并。
信号终止场景让父进程在纯用户态循环中等待 handler，检查中断返回投递路径。
完整 QEMU runner 保留其他信号、读写边界、管道、sleep 和 BusyBox 回归。

2026-09-10 实测通过：kernel check、独立用户程序 release 构建、完整镜像构建，
以及单核/4 核 QEMU 新增 SIGCHLD 测试和完整 runner。
构建日志为 `build/chld-{check,user-build,image-build}.log`；运行日志为
`build/wait4-qemu-1.log` 和 `build/wait4-qemu-4.log`。

## 未完成边界

仍针对单线程用户进程。SIGCHLD 显式 SIG_IGN 与 SA_NOCLDWAIT 的自动回收、
SA_NOCLDSTOP、停止/继续、孤儿收养及完整多线程退出需后续实现。
si_uid 当前为单 UID 模型的 0，si_utime/si_stime 尚无进程 CPU 记账，保持 0。
此批没有宣称完成全部 POSIX 信号或作业控制语义。
