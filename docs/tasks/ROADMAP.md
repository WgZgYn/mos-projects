# MOS 项目执行路线图

## 最终方向

MOS 的近期产品目标是：实现一个安全、可测试的 Linux x86_64 内核 ABI 子集，能够
运行静态链接的 musl C 程序和 BusyBox。这里的“支持 musl”指兼容 musl 依赖的
syscall、ELF、TLS、进程、文件和信号 ABI，不是在内核中实现 libc。

根目录 `roadmap-musl-busybox.md` 记录功能兼容阶段；本文件负责把功能开发、内核
正确性、工程配置、测试和项目治理编排成可执行顺序。

## 工作流分类

### 2026-09-10 当前演化顺序（按用户最新功能优先偏好）

以下是当前执行顺序；后文 R0–R6 保留为总体能力分类，不作为继续编码前必须全部完成的串行门槛。
只修复会直接阻塞当批功能的基础问题，其余技术债同步登记。

| 顺序 | 本阶段目标 | 退出条件 |
| --- | --- | --- |
| 1：shell 信号/I/O | wait4 重启已完成；补 SIGPIPE、可中断 pipe/tty/sleep 与 SIGCHLD | 管道关闭正确退出、阻塞任务响应信号，单核/4 核通过 |
| 2：前台 Ctrl-C | 提前取任务 12 的会话/进程组部分，加持久键盘输入和 TTY 前台组 | BusyBox sh 的忙循环、sleep、cat、管道 Ctrl-C 后提示符恢复 |
| 3：终端前端 | 任务 14：真实 termios、行规程、ANSI 字符模型与日志分离 | 彩色 ls、格式化输出、编辑和滚屏可靠 |
| 4：磁盘副线 | 任务 15：VFS 最小契约、块设备、只读挂载，再逐步持久写入 | 保留内存 FS 回归，最终重启验证磁盘数据 |
| 5：后续扩展 | 完整停止/继续作业控制、procfs、多线程和性能优化 | 按独立场景推进，不阻塞前面可见功能 |

TTY 与文件系统设计入口：[任务 14](14-tty-terminal.md)、[任务 15](15-persistent-filesystem.md)。
本轮检查结论：VFS 分层保留，磁盘持久性接口待补；Ctrl-C 依赖独立输入处理和前台进程组，
不能只把键盘字节映射到当前进程。完整“信号支持”分阶段验收，不一次性宣称覆盖所有语义。

| 类别 | 目的 | 对产品功能的影响 | 任务 |
|---|---|---|---|
| 配置与项目治理 | 安全交接、固定源码与工具链、明确支持范围 | 间接但前置 | 01、02、08 |
| 工程与质量基础设施 | 可靠构建、自动测试、兼容度量 | 间接但前置 | 03、04、09 |
| 内核安全与正确性 | 防止用户态破坏内核，保证 SMP/页表/调度一致 | 基础能力 | 05、06 |
| Linux/musl 功能开发 | 增加 syscall 和用户态可观察能力 | 直接 | 07、10、11、12 |
| 优化与技术债 | 降低复杂度、移除 panic 路径、基准驱动性能改进 | 持续改善 | 13 |

## 路线总览

### R0：项目可安全交接

任务：01、02。

完成定义：

- 没有可能被误提交的真实凭据；
- 新机器可以获取主仓库和全部嵌套源码；
- 项目固定 nightly、components 和 targets；
- 根 README 的命令能够从全新克隆开始执行。

### R1：建立可信工程基线

任务：03、04、08。

完成定义：

- 构建器不会吞掉子进程失败或复用意外的旧产物；
- 宿主测试、单核 QEMU、4 核 QEMU 有独立入口；
- 串口测试有结构化 PASS/FAIL、panic 检测和超时；
- UEFI/BIOS、APIC/PIC、SMP 支持矩阵与代码一致。

### R2：冻结基础 musl ABI 契约

任务：09。

完成定义：

- 有按 syscall/结构体/errno/场景组织的兼容矩阵；
- 静态 musl C 探针覆盖启动、malloc、文件、目录、时间、fork/exec/wait 和 pipe；
- 已完成能力有自动回归，未完成能力明确返回 `ENOSYS` 或正确 errno；
- 能可靠复现 L1/L2，不再依赖人工描述“曾经运行成功”。

### R3：加固内核边界

任务：05、06。

完成定义：

- syscall 不直接信任或解引用用户裸指针；
- 非法地址稳定返回 `EFAULT`；
- COW、mmap、mprotect、munmap 具有正确的跨 CPU TLB 一致性；
- 上下文切换没有 IRQ 可重入窗口；
- 4 核压力测试无 PF/GP/DF、死锁和数据串写。

R3 原则上应在继续大规模扩充 syscall 前完成，否则每个新增 syscall 都会复制不安全的
用户指针处理方式。

### R4：达到可用的 BusyBox 交互环境

任务：07。

对应兼容等级：L3。

完成定义：

- wait4 支持指定子进程、任意子进程、WNOHANG 和状态编码；
- 有可回收的 zombie 生命周期；
- SIGCHLD、SIGPIPE、SIGINT 和最小 signal frame 可用；
- BusyBox shell 能执行前台子进程，Ctrl-C 不杀死 shell；
- stdin/stdout/stderr 均正确初始化。

### R5：覆盖常用静态 C 程序和 BusyBox applet

任务：10。

对应兼容等级：L4 的基础部分。

完成定义：

- poll/select、getrandom、rlimit、sysinfo、times 可用；
- wall clock、文件时间戳和常用 tty 查询满足 BusyBox；
- `timeout`、`read -t`、`stty`、`date`、`free` 等目标 applet 通过；
- syscall 行为由 C 探针和 BusyBox 场景共同验证。

### R6：基础 pthread

任务：11。

对应兼容等级：L5。

完成定义：

- clone 的共享/复制标志组合满足 musl pthread；
- futex WAIT/WAKE 及超时语义可用；
- TLS、set/clear child tid、exit_group 语义正确；
- pthread create/join、mutex、condvar 探针在单核和 4 核通过。

### R7：系统视图和基础作业控制

任务：12。

完成定义：

- 最小 `/proc` 支持目标 BusyBox `ps/free/mount`；
- 进程组、session 和前台 tty 模型一致；
- shell 可执行基础作业控制场景。

网络栈、持久化文件系统和动态链接不属于 R0-R7 的基础 musl 主线，应分别立项。

## 每个兼容阶段的统一验收门

1. **ABI 对照**：syscall 号、参数宽度、结构体 layout、alignment、errno 有来源说明。
2. **正向测试**：静态 musl C 探针成功。
3. **负向测试**：NULL、非法地址、非法 flags、边界长度返回正确错误。
4. **应用测试**：至少一个 BusyBox 或真实小程序场景。
5. **并发测试**：涉及进程、内存、等待、信号时同时跑单核和 4 核。
6. **回归记录**：更新兼容矩阵和根 `roadmap-musl-busybox.md` 的勾选状态。

## 版本里程碑建议

| 版本 | 内容 | 必须完成任务 |
|---|---|---|
| `v0.1-musl-baseline` | 可复现地运行静态 musl 基础 C 程序和 BusyBox 基础命令 | 01-06、08、09 |
| `v0.2-busybox-shell` | 可用交互 shell、wait4 和最小信号 | 07 |
| `v0.3-linux-utility` | 常用 libc syscall 和更多 BusyBox applet | 10 |
| `v0.4-pthread` | 基础静态 pthread | 11 |
| `v0.5-process-env` | 最小 procfs 和作业控制 | 12 |

任务 13 不单独定义功能版本；它以小批次穿插进行，每批必须有测试或基准证明收益。

## 2026-09-12 调整：先补前端资源基础与实机可诊断性

本批完成窗口 owner 检查、进程镜像资源清理、帧引用固定和重绘失效传播；修复
chdir/getcwd/fchdir、/mem 挂载方向与 shell cd 句柄泄漏。详情见
[窗口生命周期与目录 ABI](../learning/frontend-lifecycle-cwd.md)。

按最新需求，优先顺序细化为：
1. 实机 BusyBox sh 无输出复测，扩大到 8/16 核 QEMU，保留明确命令输出与退出码证据。
2. 独立字符单元/UTF-8/ANSI 状态模型，输出后端与 TTY 行规程分离；再补基础色彩和编辑。
3. 窗口事件、句柄/映射完整释放契约；通过 PTY/事件接口承接用户态终端。
4. 回归原磁盘与常用 Linux 基础能力路线，完整窗口装饰及高级色彩细节后补。

本批没有把实机问题、完整资源回收或 ANSI 终端标记完成。

## 2026-09-13：终端字节流与可恢复背景

新增独立 mos_terminal 模型，基础 ANSI 颜色/编辑、跨 write UTF-8、真实行列数和
OPOST/ONLCR 已接入。终端作为合成器的可重建背景，窗口关闭后恢复被覆盖文字；
普通日志与用户终端分离。详情与支持边界见 [终端模型](../learning/terminal-model.md)。

后续优先：方向键等输入序列 → 就绪通知/poll 与 PTY 最小契约 → 用户态终端适配。
字体细节、完整 VT、复杂窗口装饰仍后补，不影响磁盘与 Linux 基础能力主线。
