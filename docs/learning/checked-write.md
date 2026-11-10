# 受检 write/writev：先复制用户输入，再进入文件 I/O

## 问题与方法

原 write 直接把用户地址转换为 Rust 切片，writev 直接解引用 iovec 指针。无效地址或
跨页权限可能导致内核异常；iovec 的非空 NULL 数据指针甚至被静默跳过。

`mos_syscall/src/fs/write.rs::writable_file` 先取得文件强引用并检查写权限。
`write_user` 使用 4 KiB 内核缓冲，以用户页边界分段调用 copy_from_user，随后进入
write_file。设备等待期间没有指向用户内存的 Rust 引用；缓冲大小不随用户长度增长。

每段写入后记录实际字节数；后续复制或设备失败，已有进展则返回字节数，否则返回错误。
继续复用 SIGPIPE 处理。write 总量限制为 0x7ffff000；零长调用检查 fd，但不访问用户地址。

`mos_syscall/src/fs/iovec.rs::sys_writev` 在输出前受检复制全部描述符到内核 Vec：

- 最多 1024 个 iovec；地址加法、总长度和有符号范围溢出均检查，支持未对齐数组。
- 描述符分配失败返回 ENOMEM，不按用户数据长度申请整块内核内存。
- 描述符错误发生在输出之前；数据缓冲错误则保留前面已完成的向量输出。
- 仅跳过长度为零的向量，非空 NULL 数据地址返回 EFAULT。

## 测试

`test_write_boundary` 验证 NULL/未映射/高半区/溢出地址、零长访问、无效或不可写 fd、
IOV_MAX、总长度溢出、未对齐描述符、跨页部分成功和描述符预检查无副作用。
完整 QEMU runner 保留 SIGPIPE、musl writev、BusyBox yes/head 管道及全部信号回归。

2026-09-10 验证：全量用户程序和完整 release 镜像构建成功；单核/4 核 QEMU 均通过
write-boundary、信号/TLS/SIGPIPE 与 BusyBox 管道/sh/echo 全部回归，差异空白检查通过。

## 后续边界

首次回归发现 shell 的未驻留只读字符串无法输出，因此同步补充 copy_from_user 的文件页
载入：先在 VMA 锁外填充独占物理页，再校验文件身份、偏移、范围和权限，最后安装 PTE。
若其他缺页路径先安装，释放本次未发布页面；所有输入复制仍在锁内重新验证后经物理映射完成。
新增 mmap 可执行文件但不先读取它、直接 write ELF 魔数的回归，防止依赖用户预触页。

- read/readv 和 ioctl 仍需受检缓冲协议；读端不能因复制失败不可恢复地吞掉管道数据。
- 输入复制已支持未驻留文件页；输出复制和普通硬件缺页路径仍需统一该填充/发布协议。
- 按页/向量进入 FileOps 尚不提供 PIPE_BUF 范围多向量写入的整体原子性；下一批结合
  管道容量、等待队列和可中断语义处理，不能用边界测试代替并发原子性测试。
- 旧 TTY 按单次写入解析 UTF-8，任务 14 需改为跨写入流式解析。

本批是受检输出阶段，不代表通用可中断 I/O 或前台 Ctrl-C 已完成。
