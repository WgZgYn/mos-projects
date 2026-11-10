# SIGPIPE、真实 shell 管道与 TLS 切换修复

## 本批问题

管道没有读者时，原实现只返回 BrokenPipe/EPIPE，没有产生 SIGPIPE。musl stdio 使用
writev，必须与 write 共享处理，否则 BusyBox 不同 applet 会表现不一致。

## 方法与核心代码

`mos_fs_runtime/src/pipe.rs` 继续只报告 BrokenPipe，VFS 不依赖 Linux 信号。
`mos_syscall/src/fs/write.rs::write_file` 将写入失败转换为调用线程的 SIGPIPE pending，
write 与 writev 共用此入口。写操作退出后才进入已有信号投递，管道锁不会跨 handler 持有。

- 默认处置：写端按 SIGPIPE 终止，wait4 的状态低位为 13。
- SIG_IGN：不排队，write/writev 返回 EPIPE。
- 自定义 handler：先运行 handler，再把 EPIPE 返回调用者。
- 屏蔽 SIGPIPE：返回 EPIPE，保留一个 pending；重复失败合并，解除屏蔽后投递。
- 零字节管道读写立即返回 0；空 write 不生成 SIGPIPE，syscall 不为它构造空地址 Rust 切片。
- writev 前面的 iovec 已写入、后面的 iovec 失败时，返回已写入字节数，同时保留 SIGPIPE。

对照依据：[Linux pipe.c](https://github.com/torvalds/linux/blob/master/fs/pipe.c) 中
anon_pipe_write 的空写、无读者 SIGPIPE/EPIPE 和部分写入路径。

## 真实管道发现的 TLS 问题

BusyBox `sh -c '/busybox yes pipe_signal_line | /busybox head -n 1'` 初次回归时，
管道能输出一行，但 shell/fork 子进程发生用户态缺页。对本地未剥离 BusyBox 反汇编后，
故障指令分别位于 `__errno_location` 和 `_Fork`，均读取 `fs:...`，实际 FS base 错为 0。

原调度代码在 `__switch_context` 返回后的 Rust 语句中读取 `new_ctx.fs_base`：
此时恢复的是该线程以前切出去时的栈，局部 new_ctx 指向的是它当时切往的另一个线程，
并不代表现在正在运行的线程。新 fork 子进程直接进入 intr_exit，甚至不经过这个 Rust 返回点。

修复在 `mos_arch/src/asm.rs::__switch_to` 中执行：RSI 仍指向真正的目标 ThreadContext，
先恢复 IA32_FS_BASE，再恢复栈和 RFLAGS。`ThreadContext.fs_base` 的 0x240 偏移由编译期
断言约束。移除原 Rust 后处理，保证新任务首次进入和已有任务恢复都走同一条正确路径。

ARCH_SET_FS 同时更新 TCB 保存值与硬件寄存器时关闭中断，避免中间被抢占后恢复旧值；
高半区/非规范用户地址返回 EINVAL，零地址允许用于清除 FS base。

## 测试入口

- `test_sigpipe`：write/writev 各 4 种处置；零长输出；pending 合并；512 字节已写入后
  读端关闭，后续 iovec 失败仍报告部分成功；真实 BusyBox yes/head 管道和 shell 退出状态。
- `test_fork_tls`：8 轮 fork，子进程首次返回读取继承 TLS；父子写不同的 COW 数据，
  多次 yield/sleep 后验证没有串用 FS base；非法 ARCH_SET_FS 不破坏原 TLS。
- 完整 runner 同时保留 wait4、先前全部信号测试和 BusyBox sh/echo。

2026-09-10 最终验证：全量用户程序及完整 release 镜像构建成功，4 个 ABI 宿主测试通过。
最新镜像在单核/4 核 QEMU 中均通过所有原有回归、test_fork_tls、test_sigpipe 和真实
BusyBox yes/head 管道。日志见 `build/wait4-qemu-1.log`、`build/wait4-qemu-4.log`。

## 保留边界与下一步

本批并未完成 Ctrl-C。它还依赖可中断 pipe/tty/sleep、持续键盘处理、前台进程组和
真实 termios。read/write/writev 的非零用户缓冲仍有历史裸指针路径，需在 I/O 扩展阶段
优先迁移；writev 总长度/IOV_MAX/PIPE_BUF 原子性也不能用本批部分写入修复代替。

完整信号的多线程退出与实时队列仍未完成。TTY 与文件系统的阶段安排分别见任务 14、15。
