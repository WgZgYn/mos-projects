# mos-projects

`mos-projects` 是 MOS 内核及其配套项目的可复现父仓库。`mos` 与 `libs/vcode` 已作为普通目录纳入本仓库，因此全新克隆不依赖本机已有的子仓库状态或远端配置。

## 环境

- Windows、Linux 或 macOS
- Git
- Rustup
- QEMU（`qemu-system-x86_64`）
- `clang`（构建相关 C 代码时需要）

仓库通过 [rust-toolchain.toml](rust-toolchain.toml) 使用最新 nightly，并自动安装 `rust-src`、`llvm-tools-preview`、`rustfmt` 和 `x86_64-unknown-none` target。nightly 是滚动版本，后续更新可能带来编译器行为变化。

## 获取与构建

```powershell
git clone <仓库地址> mos-projects
cd mos-projects
git submodule status
rustup show active-toolchain
cargo --version
cargo metadata --locked --no-deps --format-version 1
cargo check --locked
cargo build --release --locked
```

当前仓库不再使用 gitlink；`git submodule status` 没有输出是正常的。

## QEMU

具体启动参数以 `mos/README.md` 和仓库现有 Makefile 配置为准。运行前确认 `qemu-system-x86_64` 已加入 PATH。构建输出位于 Cargo 的 `target/` 目录，该目录是本机构建缓存，不需要提交。

## 常见问题

- `cargo` 没有使用 nightly：确认在仓库根目录执行，或运行 `rustup show active-toolchain`。
- 缺少 `rust-src` / target：执行 `rustup toolchain install nightly --profile minimal`，再按 `rust-toolchain.toml` 补齐组件。
- 找不到 QEMU 或 clang：安装对应平台软件并将其加入 PATH。
- 全新环境首次构建失败：先删除本地 `target/` 后重试；不要复制其他机器的 `target/` 或 `build/`。
