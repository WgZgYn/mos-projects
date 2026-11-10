# 任务 10：补齐基础 musl/Linux runtime syscall

**类别：Linux 兼容功能开发 / 内核开发编码**

## 可直接交给新对话的任务指令

在 wait4/信号主路径稳定后，补齐静态 musl 和常用 BusyBox applet 所需的下一批 Linux
x86_64 syscall。按小批次实现，每批都更新兼容矩阵并增加 C 探针；不要一次性提交所有
syscall，也不要用无语义的“全部返回 0”伪装兼容。

## 实施批次

### 批次 A：I/O 等待

- `poll(7)` / `ppoll(271)` 最小能力；
- `select(23)` / `pselect6(270)` 可基于统一等待抽象；
- tty、pipe 和普通文件的 readiness 语义；
- timeout、EINTR 和空集合睡眠。

目标场景：BusyBox `read -t`、`timeout`、管道等待。

### 批次 B：进程和资源查询

- `getrlimit(97)`、`setrlimit(160)` 或 `prlimit64(302)` 的合理最小模型；
- `sysinfo(99)`；
- `times(100)`；
- 明确哪些资源限制是固定值、软限制或暂不支持。

目标场景：BusyBox `free`、资源查询类 C 程序。

### 批次 C：随机数与时间完整性

- `getrandom(318)`，连接现有 RNG 并明确阻塞/非阻塞语义；
- wall clock 精度、启动后时间推进；
- inode atime/mtime/ctime 的最小维护；
- 补充需要的 `clock_getres` 或相关时间 syscall，以探针需求为准。

目标场景：musl `getentropy`、BusyBox `date`、`ls -l`。

### 批次 D：tty 兼容完善

- 审核 TCGETS/TIOCGWINSZ 的真实结构体 layout；
- 根据探针补充最小 TCSETS/终端状态；
- ANSI 输出由终端/控制台层正确处理或明确能力边界。

目标场景：BusyBox `stty`、交互提示和行编辑。

## 主要文件

- `mos_abi/src/syscall.rs`
- `mos_abi/src/fs.rs`
- `mos_abi/src/time.rs`
- `mos_syscall/src/lib.rs`
- 可新增 `mos_syscall/src/poll.rs`、`resource.rs`、`system.rs`
- `mos_syscall/src/time.rs`
- `mos_syscall/src/random.rs`
- `mos_fs_runtime/src/pipe.rs`
- `mos_vfs` 和 `vfs_traits`
- `mos_devfs/src/tty.rs`
- `fs/ramfs`
- 任务 09 创建的兼容矩阵和 C 探针

## 非目标

- 不实现 clone/futex/pthread；由任务 11 负责。
- 不实现 procfs 和作业控制；由任务 12 负责。
- 不为了通过探针对明显需要状态的 syscall 无条件返回成功。

## 验收标准

- 每个新增 syscall 的编号、结构体、flags、errno 与 Linux x86_64 ABI 一致。
- 每个批次有正向、超时/错误和非法指针测试。
- readiness 与现有 wait queue/scheduler 集成，不使用全局忙等。
- 目标 BusyBox applet 在自动 QEMU 测试中通过。
- 兼容矩阵和根 roadmap 同步更新。

## 验证方式

- C 探针：poll/select timeout、rlimit、sysinfo、times、getrandom、termios/winsize、文件时间戳。
- BusyBox：`read -t`、`timeout`、`free`、`date`、`ls -l`、`stty`。
- 单核和 4 核执行；涉及阻塞时增加并发 pipe/tty 压力测试。

