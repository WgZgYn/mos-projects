# MOS 后续开发任务索引

本目录中的每个文件都是一个可独立交给新对话执行的任务包。新对话开始时，可直接要求：

> 请读取 `docs/tasks/<任务文件>`，严格按其中的范围、验收标准和验证方式完成任务。先检查现状，再实施；不要处理任务范围外的问题。

产品目标以根目录 [`roadmap-musl-busybox.md`](../../roadmap-musl-busybox.md) 为准；
跨类别的详细里程碑见 [ROADMAP.md](ROADMAP.md)。

## 按性质分类

### A. 配置、治理与项目管理

这类任务不直接增加内核功能，但决定项目是否安全、可交接、可复现。

| 任务 | 优先级 | 产出 |
|---|---|---|
| [01 凭据处置](01-credential-hygiene.md) | P0 | 本地秘密隔离、轮换清单、历史扫描 |
| [02 可复现基线](02-reproducible-baseline.md) | P0 | 工具链、源码来源、README、全新克隆基线 |
| [08 启动模式与 feature](08-boot-modes-and-features.md) | P1 | 支持矩阵、UEFI/BIOS/APIC/PIC/SMP 配置收敛 |

### B. 工程基础设施与质量管理

这类任务主要改善构建、验证、持续集成和后续开发效率。

| 任务 | 优先级 | 产出 |
|---|---|---|
| [03 构建与 runner](03-build-and-runner.md) | P1 | 可靠构建、错误传播、可配置 QEMU |
| [04 自动化测试](04-test-infrastructure.md) | P1 | 宿主测试、QEMU 回归、CI |
| [09 兼容矩阵与 C 探针](09-compatibility-matrix-and-c-probes.md) | P1 | Linux ABI 清单、musl C 探针、完成定义 |

### C. 内核正确性、安全与优化重构

这类任务主要修复高风险技术债，为继续扩展 Linux ABI 提供安全基础。

| 任务 | 优先级 | 产出 |
|---|---|---|
| [05 用户内存边界](05-user-memory-boundary.md) | P0 | copyin/copyout、EFAULT、资源回滚 |
| [06 SMP 内存与调度](06-smp-memory-scheduler.md) | P0 | TLB 一致性、页表互斥、安全上下文切换 |
| [13 技术债与性能优化](13-technical-debt-optimization.md) | P2 | TODO 收敛、模块边界、基准驱动优化 |

### D. musl/Linux 兼容功能开发

这类任务直接增加可观察的用户态能力，是产品功能主线。

| 任务 | 目标等级 | 优先级 | 产出 |
|---|---|---|---|
| [07 wait4 与信号](07-process-signals-wait4.md) | L3 | P1 | BusyBox 交互 shell、Ctrl-C、SIGCHLD/SIGPIPE |
| [10 基础 musl/Linux runtime](10-basic-musl-linux-runtime.md) | L3-L4 | P1 | poll/select、rlimit、sysinfo、times、getrandom、时间戳 |
| [11 pthread：clone/futex](11-pthread-clone-futex.md) | L5 | P2 | 静态 musl pthread 最小能力 |
| [12 procfs 与作业控制](12-procfs-job-control.md) | L4+ | P2 | `ps/free/mount`、进程组和基础作业控制 |

## 推荐执行顺序

当前编码顺序优先遵循 [ROADMAP 的 2026-09-10 更新](ROADMAP.md)：信号/I/O → 前台 Ctrl-C
→ TTY/ANSI 与日志分离 → 磁盘文件系统。下表为原始总体波次，治理/优化不作为每次新增功能的门槛。
新增任务：[14 TTY 与终端前端](14-tty-terminal.md)、[15 VFS 与磁盘文件系统](15-persistent-filesystem.md)、
[16 块设备与磁盘文件系统路线图](16-block-device-and-disk.md)（任务 15 的可执行细化）。

| 波次 | 任务 | 进入条件 | 完成后的可见结果 |
|---|---|---|---|
| 0：治理止血 | 01、02 | 无 | 凭据风险受控，新对话可复现环境 |
| 1：工程基线 | 03、04、08 | 02 完成 | 一键构建、QEMU smoke、明确支持模式 |
| 2：兼容度量 | 09 | 03、04 完成 | 知道 musl/Linux 已支持什么、缺什么 |
| 3：安全底座 | 05、06 | 04 提供回归入口 | 无效用户程序不拖垮内核，SMP 行为可证明 |
| 4：交互环境 | 07 | 05 完成，建议 06 完成 | BusyBox shell 可 wait、收信号和响应 Ctrl-C |
| 5：常用 libc | 10 | 07 主路径稳定 | 更多 BusyBox applet 和普通静态 C 程序可用 |
| 6：多线程 | 11 | 05、06、07 完成 | 基础 pthread create/join/mutex/condvar |
| 7：系统视图 | 12 | 07、10 完成 | `ps/free/mount` 和基础作业控制 |
| 持续优化 | 13 | 04 完成后按批次进行 | 技术债下降，性能变化有基准数据 |

## 并行建议

- 任务 1 与任务 2 可以分别执行，但任务 1 涉及真实凭据轮换，需要用户配合。
- 任务 3 完成后再做任务 4；任务 09 依赖两者提供可信的兼容测试入口。
- 任务 5 与任务 6 可由不同对话并行，但合并前必须重新跑完整 QEMU 回归。
- 任务 8 与内核功能开发相对独立，但依赖可靠的构建和 smoke test。
- 任务 13 只能做有基准或明确正确性收益的优化，不应阻塞 musl/Linux 功能主线。

## 所有任务的共同规则

1. 开始前运行 `git status --short --branch`，保留用户已有修改，不覆盖无关内容。
2. 先读取任务涉及代码和现有文档；不要根据旧路线图直接假定问题仍存在。
3. 只修改任务列出的范围；发现范围外问题时记录，不顺手扩大改动。
4. 禁止把令牌、密码、私钥或本地机器配置写入提交内容、日志或回复。
5. 使用 `apply_patch` 编辑文件。不要使用破坏性 Git 命令。
6. 验证命令必须记录结果；无法执行时说明环境阻塞及已完成的替代检查。
7. 完成后再次报告 Git 状态、修改文件、测试结果、残余风险和建议的后续任务。
