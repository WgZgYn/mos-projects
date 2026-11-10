# 任务 05：安全的用户内存访问边界

**类别：内核开发编码 / 安全加固 / 架构重构**

## 可直接交给新对话的任务指令

为所有系统调用建立统一、可审计的用户内存访问层，消除直接把用户传入整数转换为 Rust 引用、slice、CStr 或结构体指针的做法。无效用户地址必须返回 Linux 风格 `-EFAULT`，不得造成内核 panic 或页错误。此任务安全优先，不顺便扩展 syscall 功能。

## 背景

### 2026-09-10：受检 read/readv 与事务式消费

- read/readv 已移除用户裸切片、复用描述符快照检查；先准备目标，再通过回调写回。
- 管道 peek → 受检输出 → 消费；普通文件复制成功才推进偏移，TTY 保存待交付字节。
- 未驻留文件目标在设备锁外准备，COW 写回继续使用 copy_to_user。
- 新增 test_read_boundary 和环形缓冲 peek 宿主测试，详见[受检读取](../learning/checked-read.md)。
- 下一步为可取消等待与 syscall 中断/重启；ioctl 等剩余裸指针继续迁移。

### 2026-09-10：受检 write/writev

- write 已移除用户裸切片，使用按页的固定内核缓冲；fd/写权限先检查，后续错误保留部分成功。
- writev 在输出前复制描述符，校验 1024 上限与总长度溢出，支持未对齐描述符，非空 NULL 返回 EFAULT。
- copy_from_user 增加未驻留文件页准备：锁外填充、重新核对映射、填充后发布 PTE；
  此步骤由 shell 未触页字符串回归暴露并补齐。
- 新增 test_write_boundary，含未触页文件映射输出。学习说明：[受检输出](../learning/checked-write.md)。
- 后续继续 read/readv/ioctl、输出文件页和统一硬件缺页、PIPE_BUF 原子性及可取消等待。

当前多个 syscall 直接使用：

- `slice::from_raw_parts[_mut]`
- `CStr::from_ptr`
- 裸指针 `read/write/write_volatile`
- 未限制的 iovec 数量和长度

高风险路径包括 read/write/readv/writev/getdents/pipe/stat/time/execve/uname/ioctl/window。`sys_create_window` 还存在尺寸乘法溢出、部分失败资源回滚和任意 `out_ptr` 写入风险。

## 工作范围

1. 在合适层新增统一 API，例如：
   - `copy_from_user<T>` / `copy_to_user<T>`
   - `copy_slice_from_user` / `copy_slice_to_user`
   - 有最大长度的 `copy_cstr_from_user`
   - 安全解析 argv/envp 和 iovec
2. 验证地址属于用户空间、区间不溢出、每一页已映射且权限正确。
3. 不在 syscall 生命周期中创建指向用户空间的长生命周期 Rust 引用。
4. 明确定义 `EFAULT`，保持其他 errno 语义。
5. 迁移所有 syscall 裸用户指针访问；用搜索证明没有遗漏。
6. 为 window 创建路径增加 checked arithmetic、尺寸上限和完整 rollback。
7. 考虑缺页/COW：合法的可写用户页在 copyout 时仍应按设计工作。

## 主要文件

- `mos_abi/src/error.rs`
- `mos_mem/src/virt.rs`
- 可新增 `mos_mem` 或 `mos_syscall` 的 user-memory 模块
- `mos_syscall/src/lib.rs`
- `mos_syscall/src/process.rs`
- `mos_syscall/src/time.rs`
- `mos_syscall/src/display.rs`
- `mos_syscall/src/fs/read.rs`
- `mos_syscall/src/fs/write.rs`
- `mos_syscall/src/fs/iovec.rs`
- `mos_syscall/src/fs/dir.rs`
- `mos_syscall/src/fs/stat.rs`
- `mos_syscall/src/fs/open.rs`
- `mos_syscall/src/fs/pipe.rs`
- `mos_devfs/src/tty.rs` 及其他 ioctl 实现

## 非目标

- 不实现新 syscall。
- 不改变 VFS 业务语义，除非是正确传播 `EFAULT` 所必需。
- 不通过“捕获内核页错误后继续”替代预先验证和安全复制。

## 验收标准

- NULL、内核地址、未映射地址、只读目标、跨页半映射区间和整数溢出均返回 `-EFAULT`。
- 超长字符串、argv/envp、iovec 和窗口尺寸有明确上限。
- 所有失败路径释放已分配 frame、VMA、窗口和文件描述符。
- 用户输入不能直接形成不受控的 Rust `&`/`&mut`。
- 现有 BusyBox 基础命令和用户态回归不退化。

## 验证方式

新增用户态负面测试，至少覆盖：

1. `read(fd, NULL, 1)`；
2. `write(fd, kernel_address, len)`；
3. 跨已映射/未映射页边界；
4. 只读页作为输出缓冲区；
5. iovec 数量和总长度溢出；
6. execve 未终止字符串和循环/超长指针数组；
7. 极大窗口宽高。

全部应返回 `-EFAULT` 或明确的 `-EINVAL/-ENOMEM`，串口不得出现内核 PF/panic。
