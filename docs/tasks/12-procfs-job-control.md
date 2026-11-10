# 任务 12：最小 procfs、进程组与作业控制

## 2026-09-11：控制终端与前台 Ctrl-C

- 单 tty 控制会话、前台 PGID 和 TIOCSCTTY/TIOCGPGRP/TIOCSPGRP/TIOCGSID 已接入。
- BusyBox ash 能建立独立前台管道组；VINTR 只向前台组发送 SIGINT，后台组保持运行。
- WUNTRACED 作为退出等待标志可用，但停止/继续状态、SIGTTIN/SIGTTOU 和 procfs 仍未实现。
- 说明与测试：[控制终端与前台 Ctrl-C](../learning/tty-foreground-ctrl-c.md)。

## 2026-09-11：组信号与组等待

- kill 支持当前组/负 PGID，wait4 支持当前组/负 PGID 子进程筛选。
- setpgid/setsid 后唤醒父等待队列，防止组成员迁走后等待无法结束。
- 新增 test_group_signal，保留所有旧正 PID/TID/信号/等待回归。
- 已验证：构建/check、单核/4 核 QEMU 专项与完整回归均通过。
- 说明：[进程组信号与组等待](../learning/group-signals-wait.md)。终端关联与前台 Ctrl-C 继续推进。

## 2026-09-11：会话/进程组身份接口

- 新增 setpgid/getpgid/getpgrp/setsid/getsid，fork 继承和 exec 身份保留。
- 同锁检查子进程权限、exec 后限制、跨会话/首进程限制和已有组成员关系。
- SharedState 保留组身份，弱引用登记不会长期持有 PCB；组长退出不删除其他成员的身份。
- 测试与核心代码说明：[会话和进程组身份](../learning/session-process-groups.md)。
- 回归中修复了调度锁在保存旧现场前释放的 SMP 竞态；最终构建/check、4 个 ABI 测试、单核和两轮 4 核完整回归通过。
- 下一步接组信号/组等待、控制终端与前台组；本批不表示作业控制或 Ctrl-C 已完成。

2026-09-10 顺序调整：会话/进程组与控制终端部分提前配合任务 07/14，实现前台 Ctrl-C；
不等待 procfs 或完整任务 10 完成。procfs 和停止/继续的完整作业控制仍为后续阶段。

**类别：Linux 兼容功能开发 / VFS 与进程模型开发**

## 可直接交给新对话的任务指令

在 wait4、信号和常用 runtime syscall 稳定后，实现 BusyBox 系统查看工具和基础 shell
作业控制所需的最小 `/proc`、进程组、session 与前台 tty 模型。先列目标 applet 实际
读取的节点和 syscall，不建设完整 Linux procfs。

## 工作范围

### 进程组与 session

- `getpgrp(111)`、`setpgid(109)`、`setsid(112)`；
- PCB 中的 pid/ppid/pgid/sid 模型；
- tty 前台进程组的最小 `tcgetpgrp/tcsetpgrp` ioctl；
- Ctrl-C 等终端信号按前台进程组投递；
- wait4 对进程组选择器的必要支持。

### 最小 procfs

按 BusyBox 探针结果选择节点，候选包括：

- `/proc/meminfo`；
- `/proc/cpuinfo`；
- `/proc/uptime`；
- `/proc/mounts`；
- `/proc/<pid>/stat`、`status`、`cmdline`；
- `/proc/self`。

procfs 内容应在读取时生成，并与任务、内存和 mount 表的真实状态一致。

## 主要文件

- `mos_abi/src/syscall.rs`
- `mos_syscall/src/process.rs`
- `mos_syscall/src/fs.rs`
- `mos_task/src/process.rs`
- `mos_task/src/thread.rs`
- `mos_devfs/src/tty.rs`
- `mos_vfs`、`vfs_traits`、`vfs_common`
- 可新增 `fs/procfs`
- `mos_fs_runtime/src/lib.rs`
- 任务 09 的兼容矩阵和探针

## 非目标

- 不实现完整 Linux procfs、sysfs、权限模型或容器 namespace。
- 不在 procfs 中暴露不稳定的内核裸指针。
- 不把网络、持久化文件系统纳入本任务。

## 验收标准

- BusyBox `ps`、`free`、`mount` 在目标范围内输出有效信息。
- 进程 pgid/sid 生命周期在 fork、exec、exit 后正确。
- shell 创建前台进程组后，Ctrl-C 只影响前台 job。
- procfs 并发读取不会与进程退出产生悬空引用或死锁。
- 节点格式有测试，未支持字段使用稳定且有文档的占位语义。

## 验证方式

- C 探针覆盖 getpgrp/setpgid/setsid、前台 pgrp ioctl 和错误条件。
- BusyBox 自动测试：`ps`、`free`、`mount`。
- 并发循环读取 `/proc/<pid>/*`，同时创建和退出进程。
- 交互 shell 启动前台任务并用 Ctrl-C 中断，确认 shell 继续运行。
