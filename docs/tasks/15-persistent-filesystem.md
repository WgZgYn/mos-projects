# 任务 15：VFS 接口补齐与真实磁盘文件系统

优先级：中期副线。先检查接口并记录，交互主线达到 Ctrl-C 后逐批编码；明确阻塞当前
功能的 VFS 正确性问题可以提前修复。磁盘验证使用专用测试镜像。

> 本文件回答"VFS 接口够不够"；**块设备与磁盘的具体实现顺序、QEMU 测试矩阵与工作量评估
> 见 [任务 16](16-block-device-and-disk.md)**（2026-09-20 补充，含 ext2 选型与 virtio-blk 成本判断）。

## 当前接口是否足够

结论：现有分层可以保留，足够支撑当前内存文件与设备原型，但尚不足以表达完整的磁盘
文件系统生命周期和持久性承诺。无需先推倒 VFS；先补最小契约，再用一种磁盘格式验证。

| 已检查位置 | 现有能力 | 接入磁盘前的具体缺口 |
| --- | --- | --- |
| vfs_traits/src/lib.rs | VFS/SuperBlock/Inode/FileOps/DirOps | SuperBlock 只有 root；缺设备/挂载身份、同步和卸载契约 |
| mos_vfs/src/file.rs | 打开文件状态、共享 offset、read/write | offset 读取与更新分两次锁，需明确共享文件描述的并发偏移与追加原子性 |
| vfs_common::IoContext | nonblock 标志 | 可中断/取消/就绪通知缺口；避免让通用 FS trait 直接依赖 MOS TCB |
| mos_syscall/src/fs/stat.rs | Linux stat 输出 | inode 号取对象地址、nlink 固定 1、时间取当前时间；缺真实结构化元数据 |
| mos_vfs/src/dentry.rs | 路径缓存与 inode 查询 | lookup 失败被折叠成 NotFound，磁盘 I/O 错误不能因此变成 ENOENT |
| mos_vfs/src/mount.rs | 挂载点与根目录记录 | unmount 仅移除表项；忙挂载、挂载点恢复与脏数据处理契约待补 |
| DirOps::rename/link | 同目录 rename、插入硬链接 | 跨目录原子 rename、覆盖语义、链接计数与 EXDEV 边界 |
| mos_devfs TTY ioctl | 设备请求入口 | 用户裸指针穿过 inode 边界；改用 syscall 复制与设备侧类型化请求 |

本轮在 mos_driver/mos_platform/fs/libs 中未找到可直接接入的块设备 trait/磁盘文件系统
实现；后续实施前仍需核对依赖库与硬件枚举结果，不能把 FileType::BlockDevice 等同于磁盘驱动。

## 批次顺序

### A：补契约，以 mem_fs/ramfs 验证

- 引入独立于 Linux stat 的 inode metadata：稳定 inode id、设备/文件系统身份、类型、大小、
  权限、链接数和时间；ramfs/mem_fs 提供适配，syscall 只做 ABI 转换。
- 保留 FileOps 的内核缓冲区 read/write，把用户地址复制留在 syscall；分页 I/O 不能
  简单在持有自旋锁时睡眠。先迁移当前会被扩展的 read/write/ioctl 路径。
- 定义打开文件描述的偏移锁/追加行为，明确不可 seek 设备；在不持自旋锁等待磁盘的前提下序列化。
- 修复路径查询错误传播；用注入 I/O 失败的假 inode 区分 ENOENT 与 EIO。
- 同步/卸载先明确失败语义和只读行为，再添加实现，不能返回无效果的成功。

### B：块设备与只读磁盘挂载

- 定义容量、逻辑块大小、按块读写及 flush/error 契约，提供内存假块设备测试。
- 接入一种 QEMU 可测块设备，再选择一种磁盘格式做只读实现；镜像作为独立挂载点，
  保留 ramfs 作为启动与故障恢复来源。
- 验证 ls/cat/跨块读取/大目录/挂载点路径与损坏镜像错误，不先实现通用缓存或多文件系统框架。

### C：受控写入与持久性

- 创建/写入/截断/重命名/删除、空间分配与元数据一致性逐个测试。
- fsync/sync/卸载语义与设备 flush 对齐；重新启动 QEMU 后验证文件内容仍存在。
- 加入写入失败、磁盘满、损坏元数据与中途停止测试；明确哪些操作尚无崩溃一致性保证。
- 块缓存/页缓存统一与 mmap 文件缺页放在正确性基线之后，避免形成两个失配的脏数据来源。

## 完成定义

每批必须能通过原有 ramfs/mem_fs/BusyBox 回归，记录接口变动的迁移范围。
只有重启后从测试磁盘读回数据，才称为持久化完成；仅在内存缓存中读取成功不算。
