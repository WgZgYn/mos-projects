# 任务 04：自动化测试基线

**类别：测试质量 / CI 基础设施 / 项目质量管理**

## 可直接交给新对话的任务指令

建立可重复运行的分层测试体系，覆盖宿主单元测试、内核启动、用户态回归和 SMP smoke。先让已有测试可见且可运行，再逐步补最关键的回归；不要在本任务中修复测试暴露出的所有业务缺陷。

## 背景

- 当前共有 19 个 `#[test]`，分散在 7 个基础库文件。
- `ktests/src/main.rs` 是空程序。
- `mos/src/tests.rs` 是手工测试辅助代码，并非自动测试 harness。
- 用户态有 24 个 `test_*` 二进制，但没有自动编排、统一 PASS/FAIL 或超时协议。
- 现有 QEMU 脚本只等待一个字符串，默认字符串已过期。
- 当前没有 CI。

## 工作范围

1. 定义三层测试命令：宿主单元测试、单核 QEMU 集成测试、4 核 QEMU/SMP smoke。
2. 整理宿主可测 crate 列表，避免让 no_std 内核目标阻断纯库测试。
3. 将 `ktests` 改为有实际价值的测试入口，或删除其虚假测试定位并以更合适的 harness 替代。
4. 为用户态测试定义统一输出协议，例如 `TEST <name> PASS/FAIL`，并由宿主 runner 解析。
5. 第一批至少自动覆盖：启动、基础 syscall、文件读写/目录、fork、pipe、mmap、无效 syscall、BusyBox `echo/ls/cat`。
6. QEMU 超时、panic、PF/GP/DF、未知的测试退出码必须使测试失败并保留诊断日志。
7. 添加 CI；如公共 runner 无法运行硬件虚拟化，至少运行宿主测试和构建检查，并明确 QEMU job 的运行条件。

## 主要文件

- `ktests/Cargo.toml`
- `ktests/src/main.rs`
- `mos/src/tests.rs`
- `user/user_space/src/bin/test_*.rs`
- `tools/qemu-run.ps1`
- 可新增专用测试 runner 和 CI workflow

## 非目标

- 不在本任务中完成信号、wait4 或用户内存安全重构。
- 测试发现业务失败时，记录独立 issue/任务，不扩大当前改动。

## 验收标准

- 一条命令可运行所有宿主测试。
- 一条命令可启动 QEMU、执行确定的测试集合、自动退出并返回正确状态码。
- 测试结果能指出具体失败用例，而不仅是“marker 超时”。
- 串口日志中出现 panic、PF、GP、DF 时测试必然失败。
- 单核与 4 核 smoke 都可重复运行。

## 验证方式

根据最终 runner 命名执行，至少应等价于：

```powershell
cargo test --locked -p allocators -p bitmap -p buddy_allocator -p collections -p mem_fs -p vfs_common
.\tools\qemu-run.ps1 -Target all -KillAfterWait
```

还需验证一个故意失败的用户测试能被 runner 准确报告并返回非零状态。
