# 持续键盘输入与可中断 TTY 读取

## 问题

之前 TTY read 自己消费扫描码、解码字符，并在等待按键时持有全局输入锁。
没有进程 read 时也没有字符处理，无法支持忙循环中的 Ctrl-C。其他读取方还会被
持锁等待的读取方挡住，信号无法取消它在键盘 AtomicWaker 中的登记。

## 方法与代码入口

`mos_devfs::tty::start_input_worker` 在启动用户 init 前创建常驻内核任务。
它独占 CharSyncStream 解码状态，持续消费扫描码，将 UTF-8 字符写入 4096 字节
环形缓冲，再唤醒读等待队列。扫描码队列在调度前初始化，启动函数可重复调用。

`TtyInode::read_to` 数据为空时在输入锁下登记可中断等待，由调度器完成 Blocked
转换后释放锁；恢复后清理旧队列项，再检查数据或信号。读取暂存最多 256 字节，
受检用户复制成功后才移动 head，失败不会丢输入。零长度和 NONBLOCK 行为保留。
正常唤醒持有资源锁，信号唤醒使用每次 park 的 token；清理重新取得相同资源锁，
防止旧读登记影响后续无关等待。

`ScancodeSyncStream::next_scancode` 先登记 Waker 再复查队列，关闭首次 pop
失败到登记之间可能丢失唤醒的窗口。存在数据时清除登记直接继续。这个不可取消
等待仅由常驻工作任务使用，不再属于用户 TTY read。

## 测试

`test_tty_interrupt` 重复阻塞读取和 SIGUSR1，handler 通过确认管道回应，
检查 EINTR 不修改目标字节。SA_RESTART 场景由 runner 等 handler 标记后注入
真实 QEMU 按键 q/Enter，验证重启读取；另测试 SIGKILL 结束阻塞读后，
注入 z/Enter 并继续读取，检查清理及输入路径仍可使用。

延迟读取验证输入可保留；持续解码还依赖独立工作任务的代码结构，并不因这个测试
通过就表示穷尽了所有并发时序。完整 runner 保留全部既有回归。

2026-09-11：kernel check、独立用户程序 release 构建、完整镜像构建、单核和
4 核 QEMU 的新增 TTY 专项与完整回归均通过。构建日志为
`build/tty-check.log`、`build/tty-user-build.log`、`build/tty-image-build.log`；
运行日志为 `build/wait4-qemu-1.log`、`build/wait4-qemu-4.log`。

## 当前边界

缓冲满时丢弃整个新字符，避免半个 UTF-8，尚无溢出统计。解码仍沿用
HandleControl::Ignore。会话、前台组与 ISIG/VINTR 必须一起接入，不能把 Ctrl-C
发给当时运行的任意线程。TCGETS 仍返回旧固定配置，TCSETS 尚未保存真实状态；
canonical、编辑、回显、ANSI 和日志分离也仍是后续目标。
