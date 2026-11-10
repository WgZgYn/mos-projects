# 任务 13：技术债收敛与基准驱动优化

**类别：优化重构 / 代码质量 / 长期维护（非功能主线）**

## 可直接交给新对话的任务指令

对 MOS 技术债进行一批次、可验证的收敛。不要进行无边界“大重构”，也不要把功能
缺失伪装成优化。开始时从下列候选中选择一个紧密主题，声明范围和基线数据；一次新
对话只完成一个主题。

## 可独立选择的批次

### 进程主线发现的后续债务（2026-09-09）

- 2026-09-10 shell 管道回归修复 FS base 的切换恢复与 ARCH_SET_FS 更新竞态；
  fork 首次返回和父子反复调度用 test_fork_tls 验证。非零 read/write/writev 用户缓冲、
  IOV_MAX/总长度校验和 PIPE_BUF 原子性继续列为下一批 I/O 前置正确性工作。

- 任务 07：已将 PID 从 PCB slab 解耦，暂时单调分配、不复用且不回绕；
  后续引入受 zombie 生命周期约束的可回收 PID 分配器，并实现孤儿收养。
- 任务 05/07：wait4 status 已接入带 VMA 检查、匿名缺页和 COW 的写回；
  后续补非驻留文件页 fault-in、rusage，并迁移其他 syscall 的裸指针路径。
  写回 COW 与中断缺页 COW 尚有重复逻辑，应在统一页表锁及 shootdown ack 后合并。
- 任务 06：fork 部分失败的 COW 映射/引用计数回滚，以及逐 CPU shootdown ack
  仍待完成；本批仅保证失败路径也调用现有 shootdown。
- 任务 07/11：退出发布暂沿用单线程进程假设，后续实现最后线程退出与 exit_group。
- 任务 07/11：信号控制状态已分离为 PCB 处置表和 TCB 掩码；投递阶段需要处理
  对所有线程 pending 的 SIG_IGN 清理、解除屏蔽后的投递，以及 SIGCHLD 自动回收。
- 任务 07：handler/rt_sigreturn、正 PID/TID 跨进程信号、wait4 唤醒/EINTR 已实现；
  wait4 SA_RESTART 后续也已完成，接下来补其他 blocked 任务唤醒及所有用户返回入口。
  多线程进程的发送暂拒绝，避免默认终止只杀一个线程却发布整进程退出状态。
- 任务 07/11：目标查找暂扫描 TCB slab，且 TID 仍复用槽位，与 PID 非统一命名空间；
  后续统一主线程 TID/PID、回收约束及索引。当前只探测存活用户任务，zombie 的
  kill 探测语义及未来 UID 权限检查待补。SIGCHLD 需增加退出状态等来源元数据。
- 任务 07/09：legacy FXSAVE 栈帧已验证 SIMD 恢复与 MXCSR 边界；后续需要
  musl C raise/sigaction 探针、XSAVE/AVX、altstack 和调试标志支持。
- 任务 05：受检读写当前支持匿名页和已驻留普通文件页；非驻留文件页需要可失败、
  不在自旋锁内阻塞 I/O 的 fault-in 协议，不能简单把用户指针转换成 Rust 引用。

### A. panic/TODO 路径收敛

- 盘点 `todo!`、`unimplemented!`、不应由用户输入触发的 `panic!/unwrap/expect`；
- 用户可达路径改为明确错误；
- 真正不支持的模块通过 feature 或类型边界隔离；
- 不实现大型缺失功能，只消除意外崩溃路径。

候选文件：`fs/ramfs/src/vfs.rs`、`mos_mem/src/virt.rs`、
`mos_platform/src/acpi_handler.rs`、`mos_vfs/src/vnode.rs`、
`mos_task/src/scheduler/priority.rs`。

### B. 模块边界与依赖收敛

- 审核 `mos_syscall` 对 task/mem/vfs/platform/display 的直接耦合；
- 把 ABI 解析、业务操作和硬件/调度副作用分层；
- 清理无用 feature、死模块和重复 syscall wrapper；
- 保持外部行为不变。

### C. 日志与诊断

- 定义 boot、syscall、scheduler、fault 的日志级别和稳定事件；
- 移除热路径无条件日志；
- 保留 panic/PF/GP/DF 所需上下文；
- 让 QEMU runner 能解析稳定事件而非偶然字符串。

### D. 性能优化

只有在存在基准后选择：

- timer queue 唤醒复杂度；
- VFS 路径查找和目录遍历；
- syscall/上下文切换开销；
- frame 分配和 COW；
- 显示合成 dirty region。

先测量，再优化；必须同时报告吞吐/延迟和正确性回归结果。

### E. 历史与文档一致性

- 更新失效 README、路线图行号和已修复问题；
- 明确源码生成物来源；
- 整理 `_backup` 等不属于产品源码的本地材料；
- 补充许可证和贡献约定。涉及删除时必须先获得用户授权。

## 非目标

- 不在同一批次同时重构内存、调度、VFS 和 syscall。
- 不以减少代码行数作为唯一成果。
- 不进行没有测试或基准的性能“优化”。
- 不删除用户备份或历史资料，除非用户明确授权。

## 验收标准

- 开始前有明确基线：测试结果、计数、性能数据或依赖图。
- 改动只覆盖一个主题，并有对应回归测试。
- 功能行为保持不变，或行为变化被明确列为 bug fix。
- 性能批次给出前后数据和测试环境。
- 更新技术债清单，标明已完成、保留原因和后续负责人/任务。

## 验证方式

根据批次选择：完整宿主测试、单核/4 核 QEMU 回归、静态搜索计数、依赖图或性能基准。
任何优化都必须至少运行与其路径直接相关的回归测试。
