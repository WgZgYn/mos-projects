# 可中断管道等待

## 问题

之前信号只唤醒 wait4 队列，空管道 read 和满管道 write 仍可能永久等待。直接对 TCB
调用 wake 也不够：信号发送方保存了旧等待目标后，该线程可能已经恢复并进入下一次等待。

## 方法

`mos_task::signal::park_interruptible` 为每次 park 分配独立 InterruptWait，记录目标任务
和 active 状态。调度器锁内检查 pending/处置/掩码并发布本次等待；发送方在信号锁内
取得记录，释放资源锁后进入调度器。检查与 Blocked 转换之间不会漏掉信号。

恢复执行后先将 token 标为 inactive，再清除线程中的登记。wake_interruptible 在调度器
锁内同时检查 token 和 Blocked 状态，旧 token 不会唤醒下一次睡眠。管道恢复后在资源锁内
移除未被正常事件取走的队列项，再检查条件或退出，避免留下过期引用。

管道 read/read_to/write 先判断数据、空间、EOF/EPIPE 和 NONBLOCK；没有可完成的操作
时才进入可中断等待。被信号唤醒后重查条件，仍无数据/空间且信号可处理时返回 EINTR。
屏蔽和忽略处置不产生无意义中断。

syscall 返回路径将 read/readv/write/writev 纳入 SA_RESTART，沿用信号帧保存重启现场。
已有部分字节时返回正数字节数，不会把整次操作重启，避免重复写入或读取。

## 验证

`test_pipe_interrupt` 用独立确认管道约束时序：子进程发送信号后，必须等父 handler
回写确认，再给数据管道提供数据或空间。这能区分信号唤醒与普通数据唤醒。
读/写 × EINTR/SA_RESTART 四种组合各重复三次，另测试 SIGKILL 终止阻塞读。
完整 runner 保留全部信号、TLS、读写边界和 BusyBox 管道回归。

2026-09-10 实测：独立用户程序 release 构建、完整内核镜像构建及 kernel check
通过；单核与 4 核 QEMU 均通过上述新增测试和完整 runner。日志分别保存在
`build/wait4-qemu-1.log`、`build/wait4-qemu-4.log`。这验证的是管道等待链路，
不代表尚未迁移的 TTY 或计时等待已经具备相同行为。

## 后续

TTY 键盘 AtomicWaker、计时队列、FIFO open 等尚未迁移；各自需要移除或失效旧登记，
不能仅改变 ThreadState。SIGCHLD、前台进程组与 Ctrl-C 继续保持目标内未完成状态。
管道容量和 PIPE_BUF 原子性也仍待补，当前没有声称完成整个 Linux 管道规范。
