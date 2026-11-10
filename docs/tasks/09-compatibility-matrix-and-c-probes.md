# 任务 09：Linux/musl 兼容矩阵与 C 探针

**类别：兼容性测试 / 需求管理 / 项目质量管理（少量测试编码，不实现 syscall）**

## 可直接交给新对话的任务指令

建立 MOS 的 Linux x86_64/musl 兼容契约和静态 C 探针套件。目标是准确回答“已经支持
哪些 libc 场景、依赖哪些 syscall、边界语义是否兼容”。本任务只建立矩阵和测试，不
顺便实现缺失 syscall；测试暴露的缺陷应记录到对应开发任务。

## 工作范围

1. 新建兼容矩阵，至少包含：syscall 名称和编号、参数、返回值、errno、结构体 ABI、
   当前实现位置、测试探针、状态、已知差异和目标里程碑。
2. 从静态 musl 程序的行为出发分组，而不是只罗列 syscall：
   - 进程启动、argc/argv/envp/auxv；
   - stdout/stderr、文件 I/O、目录和路径；
   - malloc/brk/mmap/mprotect；
   - clock/time/sleep；
   - fork/exec/wait；
   - pipe/dup/fcntl/ioctl；
   - signal、poll、random、rlimit；
   - pthread 前置能力。
3. 建立小型静态 musl C 探针，每个探针只验证一个或一组紧密相关 ABI。
4. 探针必须输出结构化 PASS/FAIL，并接入任务 04 的 QEMU runner。
5. 对关键 C ABI 结构增加 size/align/offset 检查：timespec、timeval、stat、dirent64、
   utsname、iovec、termios、winsize、sigaction/sigset/sigframe。
6. 对未实现 syscall 验证返回 `-ENOSYS`，而不是退出进程或崩溃。
7. 更新根 `roadmap-musl-busybox.md`：只依据自动化结果改变完成状态。

## 建议产物

- `docs/linux-compatibility.md` 或等价机器可读矩阵
- `tests/musl-probes/` 或清晰命名的探针目录
- 探针交叉编译脚本
- QEMU 探针执行清单和结果解析
- 对 `roadmap-musl-busybox.md` 的状态同步

## 主要源码对照

- `mos_abi/src/syscall.rs`
- `mos_abi/src/fs.rs`
- `mos_abi/src/time.rs`
- `mos_abi/src/error.rs`
- `mos_syscall/src/lib.rs`
- `mos_syscall/src/process.rs`
- `mos_syscall/src/task/elf.rs`
- `user/user_lib/`
- `roadmap-musl-busybox.md`

## 非目标

- 不实现信号、poll、futex 或其他缺失功能。
- 不以 Rust `user_lib` 测试替代真实 musl C 探针。
- 不以“能编译”代替 QEMU 中真实执行。

## 验收标准

- L0-L5 每项能力都能追溯到 syscall、源码和测试。
- 已支持、部分支持、stub、缺失和明确不支持五种状态定义清楚。
- 至少覆盖 L1/L2 的静态 C 探针可自动运行并全部通过。
- 兼容矩阵中不存在“已完成但无测试”的条目；这类条目应标为待验证。
- 探针失败能定位到具体 ABI，而不是只报告 BusyBox 启动失败。

## 验证方式

1. 全新构建所有探针，确认它们是静态 musl ELF。
2. 由 QEMU runner 自动逐个执行并汇总。
3. 人为调用一个未知 syscall，确认进程继续运行且得到 ENOSYS。
4. 对每个 ABI layout 探针同时记录 C 侧 `sizeof/offsetof` 和内核侧定义。

