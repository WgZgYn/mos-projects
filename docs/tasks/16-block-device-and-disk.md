# 任务 16：块设备与磁盘文件系统实施路线图

**类别：存储栈新功能开发 / 驱动 + 文件系统**
**前置阅读：[任务 15](15-persistent-filesystem.md)（VFS 契约与缺口清单，本文件是其可执行细化）**
**创建于 2026-09-20。状态：规划阶段，尚未开始编码。**

任务 15 回答"现有 VFS 分层够不够"；本文件回答"块设备与磁盘按什么顺序做、每批做什么、
QEMU 怎么测、要多久"。两文件配套使用，接口决策以任务 15 为准。

---

## 一、目标与非目标

### 目标（按顺序达成）

1. 一个**不依赖任何驱动**的块设备抽象，并能在宿主上做单元测试。
2. 用**真实 ext2 镜像**（由 WSL `mke2fs` 生成）在 QEMU 中**只读**挂载到 `/disk`，
   跑通 `ls /disk`、`cat /disk/...`、跨块读、大目录、损坏镜像报错。
3. 用 **virtio-blk-pci** 驱动读取**同一个镜像文件**，输出与第 2 步逐字节一致。
4. 受控写入：create/write/truncate/rename/unlink + fsync/sync，并在 **QEMU 重启后读回**。
5. 明确并测试"哪些操作没有崩溃一致性保证"。

### 非目标（本路线图明确不做）

- 不做通用块缓存/页缓存统一、不做 mmap 文件页（放在阶段 5，任务 15 批次 C 已限定顺序）。
- 不做日志/事务（ext2 本身无 journal；ext4 journal 不在范围内）。
- 不做真实硬件磁盘驱动（AHCI/NVMe）、不做分区表（MBR/GPT）、不做多文件系统注册框架。
- 不实现 `mount(2)`(165) / `umount2`(166) / `statfs`(137)。因此 **BusyBox `mount`/`df`
  在本路线图内不可用**，`/disk` 由内核启动时固定挂载。这是有意的范围切割。

---

## 二、现状勘察（2026-09-20 实测，含证据位置）

### 2.1 已有的部分

| 能力 | 位置 | 说明 |
| --- | --- | --- |
| PCI 配置空间访问 | `mos/src/bus/pci.rs:56-87` | 0xCF8/0xCFC 端口 I/O，`PciConfigAccess` 已实现 `pci_types::ConfigRegionAccess` |
| PCI 枚举 | `mos/src/bus/pci.rs:5-54` | 会打印 vendor/device/class 与 6 个 BAR |
| 全物理内存映射 | `mos_mem/src/lib.rs:26-40,74,112` | `physical_memory_offset` + `phys_to_virt(phys)` |
| 连续物理帧分配 | `mos_mem/src/phys.rs:63,74` | `global_allocate_frames(n) -> Option<PhysFrameRange>` / `global_deallocate_frames` |
| 页表按需映射 | `mos_mem/src/virt.rs:510,1036` | `map_page_eagerly`、`find_free_area` |
| 挂载机制 | `mos_vfs/src/mount.rs:16`、`mos_fs_runtime/src/lib.rs:41-71` | `mount(mount_point_dentry, root_dentry)`；memfs/devfs 是挂载模板 |
| FS trait | `vfs_traits/src/lib.rs:8-118` | `VFS`/`SuperBlock`/`Inode`/`FileOps`/`DirOps`；`SuperBlock` 只有 `root()` |
| 现有 FS 规模参照 | `fs/mem_fs/src/lib.rs` 293 行；`fs/ramfs/*` 约 600 行 | 一个完整 FS 的量级参照 |
| 依赖库 | `mos/Cargo.toml:19-20` | `virtio-drivers = "0.13.0"`、`pci_types = "0.10.1"` |

### 2.2 关键事实：virtio 驱动其实"几乎不用自己写队列"

`virtio-drivers-0.13.0`（`F:\SDK\cargo\registry\src\index.crates.io-1949cf8c6b5b557f\virtio-drivers-0.13.0`）已提供：

- `src/hal.rs:78-141`：`unsafe trait Hal` —— 只需实现 `dma_alloc` / `dma_dealloc` /
  `mmio_phys_to_virt` / `share` / `unshare` **五个方法**。
- `src/transport/pci.rs:105`：`PciTransport::new::<H, C>(...)` —— **现代 virtio PCI 的
  capability 解析、common/notify/ISR/device 配置区定位都已实现**。
- `src/transport/pci/bus.rs:98,148,155,217,345`：`PciRoot::new(ConfigurationAccess)`、
  `enumerate_bus`、`bar_info`、`trait ConfigurationAccess`（只需 `read_word`/`write_word`）。
- `src/device/blk.rs:46-144,177,300`：`VirtIOBlk::new` / `capacity()` / `readonly()` /
  `read_blocks(block_id, buf)` / `write_blocks` / `flush()` —— **virtqueue 管理、
  特性协商、请求/响应结构、轮询完成全部由库完成**。
- `src/transport/mod.rs:9-11`、`src/hal.rs:1-2`：`fake` transport 与 fake HAL 存在，
  但都是 `#[cfg(test)]`，**外部 crate 用不到**——所以宿主测试要自己写假块设备。

结论：virtio-blk 的**内核侧胶水约 150–250 行**（PCI 访问 + 探测 + BAR 映射 + HAL 实现），
不是从零写一个 virtqueue 驱动。这是本路线图最重要的成本判断。

### 2.3 明确缺失

- **没有任何块设备抽象**：全仓库没有 `BlockDevice`/`read_block`/sector 相关 trait
  （`FileType::BlockDevice` 只是 VFS 类型枚举，不是驱动）。
- `mos/src/driver/virtio.rs` 是**整体注释掉的 HAL 骨架**（26 行，全部 `//`）。
- `mos/src/driver/gpu.rs` 只有 1 行注释；显示走 UEFI GOP framebuffer，不经 virtio-gpu。
- `pci_enum()`（`mos/src/bus/pci.rs:5`）**当前无调用点**，启动流程
  （`mos/src/kernel.rs:7-64`）不枚举 PCI。即 PCI 代码"能编译、未启用"。
- 没有磁盘镜像的构建目标：`src/command.rs:37-43` 的 `BuildTarget` 只有
  `User/Kernel/Ramfs/All`；`src/path.rs:5-7` 只有 ramfs 相关常量。
- 没有 `sync`/`fsync` 语义；`mos_syscall/src/fs/fcntl.rs:49-53` 的 `F_SETFL` 仍被忽略。

### 2.4 镜像制作链（Windows 主机）已实测可用

```
wsl -d Ubuntu -- mke2fs -V      → mke2fs 1.47.0 (5-Feb-2023)
wsl -d Ubuntu -- which mke2fs mkfs.ext2 gcc musl-gcc
  → /usr/sbin/mke2fs, /usr/sbin/mkfs.ext2, /usr/bin/gcc, /usr/bin/musl-gcc
```

WSL2 Ubuntu 可用（`wsl -l -v` 有 `Ubuntu`，`docker` 与 `bash.exe` 也在），
所以**不需要自己写 mkfs**：`mke2fs -t ext2 -b 1024 -d <rootdir> disk.img <blocks>`
即可生成带内容的镜像（`-d` 直接填充目录树）。这也解释了现有静态 musl BusyBox 是怎么构建的。

### 2.5 现成的内存镜像加载机制（用于"零驱动"里程碑）

- ramfs 是通过 **bootloader ramdisk** 传给内核的（`src/execute.rs:188-191`
  `set_ramdisk(RAMFS_FILE_OUT_PATH)`；内核读 `ramdisk_addr`，`mos/src/kernel.rs:51`）。
- QEMU 脚本**另外**还传了 `-device loader,file=build/ramfsout/ramfs.bin,addr=0x50000000`
  （`tools/qemu-wait4.ps1:52`，常量 `src/path.rs:7`），这一路对当前内核是冗余的。
- 因此**磁盘镜像应当走 `-device loader` 的新地址**（不占用唯一的 bootloader ramdisk），
  或作为文件塞进 ramfs。两种方案见 4.1。

---

## 三、关键设计决策

**D1 先内存块设备，再真实设备。** 阶段 2 完全不碰 PCI/DMA/virtqueue 就能验证
文件系统与 VFS 集成；阶段 3 再让 virtio-blk 读同一个镜像，形成"正确性预言机"。

**D2 新建 `mos_block` crate（no_std，不依赖任何内核 crate）。**
命名 `mos_*` 会被根 workspace 的 glob 自动纳入（`Cargo.toml:22-29`）。
理由：trait 无内核依赖 ⇒ 宿主 `cargo test` 可直接用 `Vec<u8>` 假设备测边界，
不必进 QEMU。建议 trait：

```rust
pub trait BlockDevice: Send + Sync {
    fn sector_size(&self) -> usize;                 // virtio-blk = 512
    fn capacity_sectors(&self) -> u64;
    fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError>;
    fn write_sectors(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError>;
    fn flush(&self) -> Result<(), BlockError>;      // 默认 Ok(())，virtio 用 flush 请求
}
pub enum BlockError { Io, OutOfRange, Unsupported, ReadOnly }
```

**D3 文件系统选 ext2（只读优先），不选 FAT32。** 理由：

- 索引结构规整：superblock → block group descriptor →（块位图 / inode 位图 / inode 表）→
  inode 的 12 直接 + 1/2/3 级间接块指针 → 目录是 `{inode, rec_len, name_len, type, name}`
  线性数组。没有 B 树、没有日志，非常适合作为第一个磁盘 FS。
- 语义与项目目标一致：有真实 inode 号、权限位、mtime、硬链接、符号链接 —— 而这个项目
  的 BusyBox 回归**已经在用** `symlink`/`readlink`/`ls -l`/权限。
- FAT32 的坑反而更多：FAT 链 + VFAT 长文件名（checksum、slot 顺序、UTF-16）、
  无 inode/权限/链接概念，`ls -l` 与 `ln -s` 语义会对不上。

**D4 只读先于写入，写入分批。** ext2 无日志，"写入正确"与"崩溃一致"必须分开验收：
阶段 4 只承诺"正常关停后重启数据仍在"，并在文档里明确列出**不保证**的场景。

**D5 I/O 全部是同步轮询，且严禁在关中断自旋锁内做 I/O。**
`virtio-drivers` 的 `read_blocks`/`write_blocks` 内部自旋等 used ring，
所以**不需要接 PCI 中断**（也就不需要 IOAPIC 路由、MSI-X、ACPI _PRT）。
代价是等待期间空转 CPU —— 对 QEMU 演示完全可接受，对真实硬件后续再引入中断+等待队列。
硬性规则：先做 I/O（或先做块缓存查表），再拿 VFS/进程自旋锁；
`IoContext` 不睡眠、不自旋等锁（与任务 15 批次 A 一致）。

**D6 块大小转换层是明确的一层。** virtio 扇区 512B，ext2 块大小建议 **1024B**
（镜像小、`mke2fs -b 1024` 一行搞定）。FS 层只认 1024B 块，
`read_blocks` 需要把 1 个 1024B 块拆成 2 个扇区，并处理不对齐的尾块。

**D7 元数据必须来自 FS，而不是对象地址。**
`mos_syscall/src/fs/stat.rs:37` 现在用 `Arc::as_ptr(...) as u64` 当 `st_ino`，
`:39,52-54` 三个时间戳都取"当前时间"。ext2 有真实 inode 号与 mtime，
阶段 2 必须把 `file_size`/`st_ino`/`st_mode`/`st_nlink`/`st_mtime` 接成 FS 提供的值，
否则 `ls -l`/`find`/`make` 这类程序行为会明显失真。

**D8 "测试后门"要自识别，不靠编译期配置。**
内存块设备在固定物理地址探测 ext2 magic（`0xEF53` @ offset 1024+56）：
有效才注册设备，否则当作没有磁盘。这样 QEMU 传/不传 `-device loader` 都能启动，
不需要新增 feature flag 或第二个 ramdisk 通道。

---

## 四、阶段与批次

### 阶段 0：准备（不改内核行为）

**范围**
1. **先提交现有 11 天未提交工作**（见任务 15 与本仓库审计结论）；磁盘工作应基于干净基线。
2. 新增 `tools/mkdisk.ps1`：调 WSL `mke2fs`，把 `build/diskroot/` 打成
   `build/disk/disk.img`；支持 `-Blocks`、`-BlockSize`、`-Inodes` 参数。
3. `src/path.rs` 增加 `DISK_ROOT_DIR_PATH = "build/diskroot"`、
   `DISK_FILE_OUT_PATH = "build/disk/disk.img"`、`DISK_IMG_ADDR = 0x51000000`、
   `DISK_IMG_LEN = 8 MiB`。
4. `src/command.rs` + `src/execute.rs` 增加 `BuildTarget::Disk`（并加入 `All` 的尾部）。
5. `tools/qemu-disk.ps1`：以现有 `qemu-wait4.ps1` 为模板，附加磁盘参数（见第五节）。

**产出**：一条命令生成带内容的 ext2 镜像；QEMU 能把它放进 guest 内存。
**验收**：`wsl -d Ubuntu -- dumpe2fs -h build/disk/disk.img` 显示预期块数/inode 数；
镜像 SHA256 记录在 `build/disk/disk.img.sha256`。

### 阶段 1：块设备抽象 + 内存块设备（宿主可测）

**范围**
1. 新 crate `mos_block`：`BlockDevice` trait、`BlockError`、`RamBlockDevice`
   （`Arc<[u8]>` 或 `Vec<u8>` 后端，支持 `ReadOnly` 模式 `write_sectors -> ReadOnly`）。
2. 边界检查：`lba + count` 溢出、越界、`buf.len()` 与扇区数不符、`sector_size != 512`。
3. `CachedBlockDevice`（可选，建议做）：固定 N=64 项、写通的 1024B 块缓存，
   只为减少重复读，不做预读、不做回写 —— 避免与任务 15 "先不做通用缓存"冲突。

**产出**：`mos_block` + 宿主测试 ≥ 12 条。
**验收**：`cargo test -p mos_block`；覆盖越界、溢出、只读、零长度、非对齐尾块。

### 阶段 2：ext2 只读 + 挂载 `/disk`（**第一个可见里程碑，零驱动**）

**范围**
1. 新 crate `fs/ext2`（`fs/*` glob 自动纳入）：`SuperBlock` + `GroupDesc` + `Inode`
   （含 1/2/3 级间接块）+ `DirIter`（处理 `rec_len` 与已删除项）+ 校验
   （magic `0xEF53`、`s_block_group_nr`、`s_inode_size`、块/inode 计数上限）。
2. `impl vfs_traits::{VFS, SuperBlock, Inode, FileOps, DirOps}`，镜像 `fs/mem_fs` 的写法。
3. 只读打开路径：`open(..., O_RDONLY)` 正常，写路径返回 `EROFS`（`VfsError::PermissionDenied`
   或新增 `ReadOnly`，见任务 15 批次 A 的错误传播要求）。
4. `mos/src/kernel.rs` 增加 `mos_fs_runtime::mount_ext2_from_memory()`：
   探测 `DISK_IMG_ADDR` 的 ext2 magic → 注册为 `/disk`（用 `open(..., CREATE|DIRECTORY)` +
   `mount(dentry, root)`，与 `mount_mem_fs` 同构）。
5. 用 `Inode` 真实元数据替换 `stat` 的临时值（D7）。
6. 错误区分：块设备 I/O 失败必须是 `EIO`，不能折叠成 `ENOENT`
   （任务 15 已指出 `mos_vfs/src/dentry.rs` 的折叠问题）。

**产出**：`fs/ext2`（预计 600–800 行）+ 挂载代码 + 用户态测试
`test_disk_ro.rs`、`test_disk_bigdir.rs`、`test_disk_corrupt.rs`。
**验收**（QEMU，Tier 0 参数见 5.1）
- `ls /disk` 列出镜像内容；`cat /disk/hello.txt` 内容正确；
- 跨块文件（>4 KiB）逐字节正确；512 项目的目录可完整列出且不重复/不丢项；
- 故意破坏 superblock / 位图 / 间接块后：`-EIO`，且**串口无内核 PF/panic**；
- 原有 ramfs/memfs/BusyBox 回归不退化。

### 阶段 3：virtio-blk-pci 驱动（同一镜像，输出必须一致）

**范围**
1. 新 crate `mos_block_virtio`（或 `mos_driver/src/virtio_blk.rs`，二者选一，
   建议放 `mos_driver` 以复用现有 `mos_driver` 依赖方向）。
2. `impl Hal`（5 个方法）：`dma_alloc` = `global_allocate_frames(pages)` + `phys_to_virt`
   + 清零；`dma_dealloc` = `global_deallocate_frames`；`mmio_phys_to_virt` = `phys_to_virt`
   （**注意 MMIO 的缓存属性**，见风险 R3）；`share` = `virt - physical_memory_offset`
   （加范围断言），`unshare` = 空操作。
3. `impl ConfigurationAccess`（复用 `mos/src/bus/pci.rs:59-86` 的端口 I/O 逻辑，
   适配 `read_word`/`write_word`）。
4. 探测：`PciRoot::new(...)` → 遍历 → vendor `0x1AF4`，device `0x1001`/`0x1042`
   → 启用 `BUS_MASTER` + `MEMORY_SPACE` → 取 BAR4（现代 MMIO）→ `PciTransport::new`
   → `VirtIOBlk::new` → 包成 `BlockDevice`（512B 扇区）。
5. `readonly` 协商：QEMU 传 `readonly=on` 时 `VirtIOBlk::readonly()` 为真，
   `write_sectors` 直接返回 `ReadOnly`。
6. 失败路径：无 virtio-blk 设备时**不要 panic**，记录一条日志并继续用 ramfs
   （与 `mos_driver::ps2_mouse::init_ps2_mouse` 的容错风格一致，`mos/src/main.rs:81-85`）。

**产出**：驱动胶水 150–250 行 + `test_disk_virtio.rs`。
**验收**（QEMU，Tier 1 参数见 5.2）
- 同一个 `disk.img`，`cat /disk/*` 输出与阶段 2 逐字节一致；
- `capacity_sectors()` × 512 == 镜像文件大小；
- 1 核与 4 核都通过；`-device virtio-blk-pci` 缺席时系统照常启动；
- 通过 screendump/串口断言验证**没有**出现双重挂载或设备被重复初始化。

### 阶段 4：受控写入与持久化（**时间主要花在这里**）

**范围（建议拆 4a/4b/4c/4d 四个小批，每批独立验收）**

- **4a 写入基础**：`write` 到已存在文件（覆盖、不扩长）；块位图/inode 位图只在需要时改；
  写前校验目标块范围与 `i_blocks` 一致性。
- **4b 分配**：新建文件/目录（分配 inode + 数据块）、扩长（含间接块首次分配）、
  空间不足返回 `ENOSPC` 且**回滚已完成的部分分配**。
- **4c 结构调整**：`truncate`/`ftruncate`（含释放间接块）、`unlink`、同目录 `rename`、
  目录项 `rec_len` 合并（这是 ext2 最容易被忽略的细节）。
- **4d 持久性与一致性**：`fsync`/`sync` 语义、`BlockDevice::flush` 与设备 flush 对齐；
  **重启 QEMU 后读回**；写入失败注入（见 5.4）；文档明确列出无崩溃一致性保证的操作。

**验收**
- `echo hi > /disk/a.txt` 后 `sync`，重启同一镜像（**不加 `snapshot=on`**）仍能 `cat` 到；
- 磁盘满返回 `ENOSPC`，且重启后文件系统仍可用（不能变成不可挂载）；
- 中途 kill QEMU 后，镜像仍能被宿主 WSL `e2fsck -fn` 扫过（允许报错，但必须能诊断）；
- 写入 1 MiB 文件后宿主侧 `md5sum` 与 guest 侧一致。

### 阶段 5：后续（不在本路线图承诺内）

块缓存/页缓存统一、mmap 文件页、崩溃一致性设计、AHCI（真实 SATA）、NVMe、
MBR/GPT 分区、`mount(2)`/`statfs(137)`/`umount2` 让 BusyBox `mount`/`df` 可用。

---

## 五、QEMU 测试矩阵（可直接抄的参数）

### 5.1 Tier 0：内存镜像，零驱动（阶段 2）

在现有脚本参数（`tools/qemu-wait4.ps1:46-52`）基础上**只加一行**：

```
-device loader,file=build/disk/disk.img,addr=0x51000000
```

- 与 ramfs 的 `0x50000000` 不冲突（ramfs 4 MiB，止于 0x50400000）。
- 内核按 D8 探测 ext2 magic；不传这一行则没有 `/disk`，启动照常。
- **不需要** `-device virtio-blk-pci`、不需要 IOAPIC 路由、不需要 ACPI _PRT。

### 5.2 Tier 1：真实块设备（阶段 3）

```
-drive file=build/disk/disk.img,format=raw,if=none,id=disk0 \
-device virtio-blk-pci,drive=disk0,disable-legacy=on
```

- `disable-legacy=on` 让设备为纯现代（device id `0x1042`），避免 transitional 双 BAR 的歧义；
  `virtio-drivers` 的 PCI transport 走 capability 列表（`transport/pci.rs:105`），
  与 `disable-legacy=on` 匹配。
- 启动镜像那一路保持 `snapshot=on`；**数据盘不要加 `snapshot=on`**，否则重启后数据不落盘。
- `-smp 1` 与 `-smp 4` 都要跑（沿用现有 runner 的 `-Cpus` 约定）。

### 5.3 只读与容量断言

- 只读：给数据盘加 `readonly=on`，断言 `write` 返回 `-EROFS`/`-EACCES`，
  且内核不 panic、不破坏镜像（宿主侧 `sha256sum` 不变）。
- 容量：`capacity_sectors() * 512 == (Get-Item disk.img).Length`，写进 runner 断言。

### 5.4 失败注入（阶段 4 的必测项）

- 读写错误回报：`-drive ...,rerror=report,werror=report`（默认即 report）。
- 定点注入 I/O 错误：QEMU 的 `blkdebug` 驱动
  `-drive file=blkdebug:build/disk/blkdebug.conf:build/disk/disk.img,format=raw,if=none,id=disk0`
  （在 `blkdebug.conf` 里按事件注入 `errno = 5`），用来验证 `EIO` 传播与**不 panic**。
- 损坏镜像：直接用 `tools/mkdisk.ps1` 生成后用脚本打洞（破坏 superblock/位图/间接块），
  验证 `-EIO` 与可诊断日志。
- 重启持久化：同一 runner 顺序执行「写 → 关 VM → 开 VM → 读回」，两个 VM 共用镜像路径，
  不共用 monitor 端口（现有脚本的端口冲突问题见任务 03/13）。

### 5.5 每批都要跑的门（沿用项目既有约定）

1. 宿主 `cargo test -p mos_block -p ext2`（新增）+ 现有 21 条宿主测试；
2. `cargo check` 内核 target（`x86_64-unknown-none`）；
3. QEMU 1 核 + 4 核：新增磁盘用例 + 原有 `qemu-wait4.ps1` 全量回归；
4. 串口不得出现 `PF/GP/DF/panic/user-panic`；
5. 记录镜像 SHA256 与 `disk.img` 生成参数（注意：**镜像字节不可复现**，
   现有 `build/image/mos-uefi.img` 已验证同一源码两次构建哈希不同，
   所以证据要记"源码状态 + 生成参数"，而不是把哈希当唯一凭据）。

---

## 六、工作量与复杂度评估（回答"要多久/复不复杂"）

| 阶段 | 内容 | 规模估计 | 风险 | 批次估计 |
| --- | --- | --- | --- | --- |
| 0 | 提交基线、镜像脚本、CLI/常量、QEMU 模板 | ~200 行（多为 PowerShell/CLI） | 低（但要先清理 git 状态） | 0.5 |
| 1 | `mos_block` trait + 内存设备 + 缓存 + 宿主测试 | ~250–400 行 | 低（纯逻辑，可 TDD） | 0.5 |
| 2 | ext2 只读 + 挂载 `/disk` + 元数据接入 | ~700–1000 行 | **中**：间接块、目录项 `rec_len`、错误码区分 | 1.5–2 |
| 3 | virtio-blk-pci（HAL + PCI + BAR + 容错） | ~200–300 行 | **中高**：BAR/MMIO 映射、`share` 的 virt→phys、特性协商 | 1–1.5 |
| 4 | 写入/分配/截断/重命名/持久化/失败注入 | ~400–700 行 | **高**：分配回滚、`rec_len` 合并、一致性与诊断 | 2–4 |
| 5 | 缓存统一/mmap/日志/NVMe/分区 | 远超本路线图 | 高 | — |

**结论性判断（分三层看"真实磁盘系统"）：**

- **"能读真实磁盘上的文件"（阶段 1–3）**：总量约 1200–1700 行，**4–6 个批次**量级，
  和刚做完的终端前端（`mos_terminal` 461 行 + tty 重写 328 行 + 大量 runner）相当。
  其中 QEMU 侧**没有任何新魔法**：一个 `-device loader`（Tier 0）＋ 一条
  `virtio-blk-pci`（Tier 1），`virtio-drivers` 把 virtqueue 都包了。
- **"能写、能持久化"（阶段 4）**：这才是真正长的一段，**2–4 个批次**，
  而且难点不在"读磁盘索引"，在**分配回滚、目录项合并、失败注入下的错误传播、
  以及明确说清哪些情况没有崩溃一致性**。
- **"真实硬件磁盘"（AHCI/NVMe/分区表）**：完全是另一条轴线，
  AHCI 约 1–2 批次，NVMe（queue pair、MSI-X、PRP 列表）约 3–5 批次。
  **不建议现在做**：QEMU 的 virtio-blk 已经能覆盖"真实块设备 I/O"的全部教学设计，
  而收益（在真机上跑）与成本不成比例。

### 磁盘索引到底复不复杂？

**"索引"本身不复杂，复杂的是围绕它的工程约束。** 具体拆开：

- ext2 的盘上索引就五层，且都是定长数组/线性表，**没有 B 树、没有日志**：
  superblock（块大小、块数、每组长度的计数）→ 块组描述符表
  → 每组 {块位图, inode 位图, inode 表} → inode 的 12 直接块 + 1/2/3 级间接块指针
  → 目录 = `{inode, rec_len, name_len, type, name}` 数组。
  只读遍历就是一个纯树/链式查表，非常适合第一次写磁盘 FS。
- 真正花时间的是这五件事（都不在"索引格式"里）：
  1. **块大小不匹配**：512B 扇区 vs 1024B 文件系统块，所有读写都要转换与对齐；
  2. **锁与 I/O 的边界**：文件系统 trait 里不能睡眠、不能在关中断自旋锁内做 I/O，
     而现在 VFS 层的锁粒度（`mos_vfs/src/file.rs`）是按内存 FS 设计的；
  3. **错误语义**：`EIO` vs `ENOENT` vs `EROFS` vs `ENOSPC` 必须分清楚
     （任务 15 已点名 `dentry` 会把 I/O 失败折叠成 `NotFound`）；
  4. **写路径的分配与回滚**：位图、间接块、`rec_len` 合并、`truncate` 释放；
  5. **崩溃一致性**：ext2 无日志，必须显式声明不保证的范围，不能默认"写完就在"。
- FAT32 对比：FAT 链 + 8.3 目录项看起来更简单，但 VFAT 长文件名的
  checksum/slot 顺序/UTF-16 会引入另一类细节，而且没有 inode/权限/链接概念，
  与本项目的 Linux ABI 目标不符 —— 所以 **ext2 反而是更省事的选择**。

---

## 七、风险登记

| 编号 | 风险 | 影响 | 缓解 |
| --- | --- | --- | --- |
| R1 | 在 QEMU 里把"轮询 I/O"放进关中断自旋锁内 | 高延迟、真实硬件上死锁 | D5 硬规则；阶段 2 就定 I/O 与锁的边界并写进学习文档 |
| R2 | `share()` 需要 virt→phys，而项目只有 `phys_to_virt` | 驱动拿到错物理地址 → 数据损坏 | 只对 offset 映射区内的指针做减法并加范围断言；先用 `Dma` 分配的缓冲验证 |
| R3 | MMIO 用普通可缓存映射 | QEMU 能跑，真机会出玄学错误 | 明确标注"QEMU-only 成立"，真机前必须引入 UC/PAT |
| R4 | ext2 无日志，写入中途断电 | 镜像损坏 | 阶段 4 明确"无崩溃一致性保证"；用 `e2fsck -fn` 做可诊断性验收 |
| R5 | 阶段 0 之前不清理 git 状态 | 磁盘改动与 11 天未提交内容混在一起，无法二分定位 | 先按审计建议分批提交 |
| R6 | 镜像不可字节复现 | 测试无法用哈希做唯一断言 | 断言"内容/语义"，哈希只记生成参数与来源 |
| R7 | WSL 在 CI/他人机器上不可用 | 镜像制作成为单点依赖 | `tools/mkdisk.ps1` 里检测 WSL，并保留"纯 Rust 最小 mkfs"作为后备（仅当 WSL 不可用时才做） |
| R8 | 在 `virtio-drivers` 上打补丁或依赖其内部行为 | 升级脆弱 | 只用公开 API；把 HAL/PCI 差异收在一个文件里 |

---

## 八、需要用户确认的开放决策

1. **文件系统**：ext2（推荐，理由见 D3）还是 FAT32（Windows 侧格式化更顺手）？
2. **是否要 BusyBox `mount`/`df`**：若要，需额外实现 `mount(2)`/`statfs(137)`/`umount2`，
   建议作为阶段 5 单独立项，不塞进本路线图。
3. **块缓存**：阶段 1 就加 64 项写通缓存（推荐，代码少、后续不用改接口），
   还是完全不加、阶段 5 再统一？
4. **镜像规模**：建议 8 MiB / 1024B 块（够放 busybox 子集与测试文件，`mke2fs` 秒级）。
   若要放完整静态 busybox（~1–2 MiB）与更多 applet，可提到 16–32 MiB。
5. **磁盘挂载点**：建议 `/disk`（与 `/mem`、`/dev` 同级）；是否要 `/mnt`？

---

## 九、建议的第一步（最小可验证动作，不写内核代码）

```powershell
# 1) 先保住现有工作（见审计报告的建议分批提交）
git status --short --branch

# 2) 用 WSL 生成一个带内容的 ext2 镜像
New-Item -ItemType Directory -Force build/diskroot | Out-Null
Set-Content build/diskroot/hello.txt "hello from ext2"
.\tools\mkdisk.ps1 -Blocks 8192 -BlockSize 1024     # 待阶段 0 新建
wsl -d Ubuntu -- dumpe2fs -h /mnt/f/RustroverProjects/mos-projects/build/disk/disk.img

# 3) 用 Tier 0 参数在当前内核里先"看"这个镜像
#    （此时内核还没实现 ext2，先确认 -device loader 能把镜像放进内存、
#      并能在串口打印出磁盘地址处的 magic 0xEF53）
```

第 3 步是本路线图的**第一个可观察信号**：只要串口能打印出镜像里的 ext2 magic，
就说明"镜像制作 → QEMU 注入 → 内核可访问"这条链已经通了，之后所有工作量都在
纯逻辑（解析 FS）与驱动（virtio-blk）上，风险可控。
