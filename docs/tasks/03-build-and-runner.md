# 任务 03：构建器与 QEMU runner 可靠性

**类别：开发工具 / 构建基础设施 / 可靠性优化**

## 可直接交给新对话的任务指令

修复宿主构建器和 QEMU runner，使全新工作区可以可靠构建、失败能正确向上传播、运行命令不会使用意外的旧产物。保持现有 CLI 大体兼容，不处理内核功能问题。

## 背景

- `Run.build_if_updated` 已定义但未使用，`run` 会直接启动已有镜像。
- 用户态子构建没有检查退出状态；复制失败仅记录并继续返回成功。
- 内核子构建调用 `wait()` 但没有检查退出码。
- `build/ramfsroot`、`build/ramfsout`、`build/kernel`、`build/image` 不受版本控制，代码没有统一创建这些目录。
- QEMU 路径硬编码为 Windows 安装路径。
- OVMF 使用 `Source::LATEST`，缺乏版本确定性。
- `tools/qemu-run.ps1` 默认等待已不存在的 `ready to get input`，并可能停止所有同名 QEMU 进程。

## 工作范围

1. 在构建前创建所有所需输出目录。
2. 检查每个子进程的退出状态；任何子构建或复制失败都必须使顶层命令失败。
3. 防止部分成功后留下的旧产物被误认为本次构建结果；必要时使用临时输出后原子替换。
4. 实现或移除 `build_if_updated`。若保留，定义清晰的 freshness 判定；默认行为不得静默启动旧镜像。
5. 允许通过 CLI、环境变量或配置指定 QEMU 路径，硬编码路径仅可作为 Windows fallback。
6. 固定或缓存明确版本的 OVMF，并离线复用已取得文件。
7. 更新 QEMU smoke marker，使其对应稳定的内核/用户态 ready 事件。
8. QEMU runner 只能清理自己启动或明确通过 PID/monitor 标识的实例。

## 主要文件

- `src/command.rs`
- `src/execute.rs`
- `src/path.rs`
- `user/src/lib.rs`
- `ramfs_builder/src/lib.rs`
- `tools/qemu-run.ps1`
- `Cargo.toml`

## 非目标

- 不实现新的 syscall。
- 不重构调度器、VFS 或显示系统。
- 不顺便升级全部依赖。

## 验收标准

- 删除本地构建目录后的全量构建能够自行创建目录并成功。
- 用户态、ramfs、内核或复制任一环节失败时，顶层命令返回非零状态。
- `run` 的构建策略与 CLI 文档一致，不会无提示使用陈旧镜像。
- QEMU 路径可配置，错误信息包含实际尝试的路径。
- smoke test 等待的标记在当前 shell 启动后确实出现。
- runner 不会杀死与本次任务无关的 QEMU 实例。

## 验证方式

```powershell
cargo run --release -- build --target user
cargo run --release -- build --target ramfs
cargo run --release -- build --target kernel
cargo run --release -- build --target all
.\tools\qemu-run.ps1 -KillAfterWait
```

另外增加一个可控的失败验证，例如临时指定不存在的 QEMU 或构建输入，并确认命令非零退出且不会报告成功。验证过程不得覆盖用户已有产物，优先在临时副本中完成。
