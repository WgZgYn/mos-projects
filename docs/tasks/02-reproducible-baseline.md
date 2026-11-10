# 任务 02：可复现仓库与工具链基线

**类别：工程配置 / 仓库治理 / 项目基础设施（非功能开发）**

## 可直接交给新对话的任务指令

让 MOS 项目能够从全新克隆开始，以明确且固定的工具链取得全部源码并构建。先调查嵌套仓库的真实远端和目标提交，不要猜测 URL，不要推送任何分支。若远端缺少主仓库所引用的 commit，报告阻塞并给出安全选项。

## 背景

- 根仓库把 `mos` 和 `libs/vcode` 记录为 gitlink，但没有 `.gitmodules`。
- `mos` 当前本地分支比其 `origin/master` 领先 2 个提交，主仓库引用 `1edc2ac`。
- `libs/vcode` 当前没有可见的上游跟踪信息，需要查明来源。
- 项目使用 edition 2024、`build-std` 和 `abi_x86_interrupt`，实际需要 nightly，但没有 `rust-toolchain.toml`。
- `mos/README.md` 的工具链说明不完整，根目录没有完整启动文档。

## 工作范围

1. 确认 `mos` 与 `libs/vcode` 的仓库身份、远端 URL、许可证和主仓库引用 commit 的可获取性。
2. 在“正确配置 submodule”与“将代码正式纳入主仓库”之间选择最小且可维护的方案；不允许继续保留无来源 gitlink。
3. 添加固定日期或固定版本的 `rust-toolchain.toml`，声明所需 components/targets，包括 `rust-src`、`llvm-tools-preview` 和 `x86_64-unknown-none`。
4. 编写根 README：支持的平台、依赖、工具链、构建命令、QEMU 命令、输出目录和常见错误。
5. 确保全新克隆不依赖本机已有 `target/`、`build/` 或嵌套仓库状态。
6. 不升级业务依赖，不修改内核功能。

## 主要文件

- `.gitmodules`（若采用 submodule）
- `rust-toolchain.toml`
- `README.md`
- `Cargo.toml`
- `mos/README.md`
- `mos` gitlink
- `libs/vcode` gitlink

## 决策约束

- 不得虚构 `libs/vcode` 的远端 URL。
- 不得自动 push `mos` 的 2 个本地提交；如远端不可获取，必须让用户决定先推送还是改为 vendoring。
- 固定 nightly 前应验证它能处理当前 lockfile、edition 2024 和 build-std。

## 验收标准

- 新克隆能按 README 获得所有源码。
- `git submodule update --init --recursive` 可成功执行，或者仓库已不再使用 gitlink。
- `cargo --version` 自动选择项目固定的 nightly。
- 不依赖用户级默认 toolchain。
- README 中的命令可逐条执行，不包含本机专属绝对路径。

## 验证方式

在临时目录进行全新克隆验证：

```powershell
git submodule status
rustup show active-toolchain
cargo --version
cargo metadata --locked --no-deps --format-version 1
cargo check --locked
```

若完整构建尚被任务 03 阻塞，至少证明所有源码和工具链均可取得，并明确剩余阻塞。
