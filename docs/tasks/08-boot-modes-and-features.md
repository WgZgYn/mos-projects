# 任务 08：启动模式与 feature 收敛

**类别：平台配置 / 构建配置 / 技术债收敛**

## 可直接交给新对话的任务指令

审计并收敛 UEFI/BIOS、APIC/PIC、SMP 和 ramfs 相关 feature，使 CLI 暴露的每种模式都真实可构建、可启动、可测试。若 BIOS/legacy 当前不准备支持，应删除或明确标记不可用，而不是保留会在运行时失败的选项。

## 背景

- CLI 暴露 `uefi` 和 `bios`。
- 构建器对 BIOS 选择 `intr_legacy`，对 UEFI 选择 `intr_apic`。
- 内核只有 ACPI 初始化受到 `intr_apic` 条件控制。
- `mos_platform` 固定使用 APIC controller，并在初始化过程中访问 APIC/ACPI 状态。
- 根 `Cargo.toml` 的 `uefi`、`map_ramfs` feature 没有实际条件编译用途。
- `mos_platform` 的 `smp` 默认为开启。
- trampoline `.asm → .o → .bin` 依赖手工 shell 脚本，未接入 Cargo dependency tracking。

## 工作范围

1. 列出并定义支持矩阵：UEFI/APIC/SMP、UEFI/APIC/单核，以及是否支持 BIOS/PIC。
2. 让 CLI、Cargo feature、内核 cfg 和平台 controller 选择保持一致。
3. 如果保留 BIOS：实现完整 PIC/timer/keyboard 初始化路径，禁止访问未初始化 APIC，并增加 BIOS smoke。
4. 如果暂不保留 BIOS：从可执行 CLI 中移除或明确返回“暂不支持”，避免生成不可启动镜像。
5. 删除无作用 feature，或把它们接到真实行为并增加 feature 测试。
6. 将 trampoline 生成接入 Cargo build script，声明输入文件变化，并验证输出；避免提交无法追溯的陈旧 `.o/.bin`。
7. 不改变 syscall 和用户态功能。

## 主要文件

- `Cargo.toml`
- `src/command.rs`
- `src/execute.rs`
- `mos/Cargo.toml`
- `mos/src/kernel.rs`
- `mos/src/boot.rs`
- `mos_platform/Cargo.toml`
- `mos_platform/src/lib.rs`
- `mos_platform/src/apic.rs`
- `mos_platform/src/pic.rs`
- `mos_platform/src/acpi.rs`
- `mos_platform/src/trampoline.asm`
- `mos_platform/src/build_trampoline.sh`
- 可能新增的 `mos_platform/build.rs`
- `tools/qemu-run.ps1`

## 决策要求

开始编码前先给出支持矩阵和选择理由。若 BIOS 实现工作量明显超出本任务，应选择“明确禁用 BIOS”并创建独立后续任务，而不是留下半实现状态。

## 验收标准

- CLI 不再允许选择会确定失败的组合。
- 每个声明支持的组合都能通过编译和 QEMU smoke。
- 单核模式不启动 AP，不依赖 SMP trampoline。
- APIC/PIC controller 由 cfg 或类型明确选择，不是运行时访问未初始化全局对象。
- trampoline 源文件变化会自动触发重新生成，生成失败使构建失败。
- 无效或冗余 feature 被移除或有测试覆盖。

## 验证方式

按最终支持矩阵执行至少：

```powershell
cargo run --release -- build --target all --boot-mode uefi
.\tools\qemu-run.ps1 -KillAfterWait
```

另行验证单核和 4 核。如果声明支持 BIOS，还必须构建并启动 BIOS 镜像，验证 timer、键盘、串口、init 和 shell；不能只验证镜像生成成功。
