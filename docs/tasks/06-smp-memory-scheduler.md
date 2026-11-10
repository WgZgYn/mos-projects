# 任务 06：SMP 页表与调度安全

**类别：内核开发编码 / 正确性优化 / 并发架构重构**

## 可直接交给新对话的任务指令

完成一次聚焦 SMP 的正确性修复：页表修改必须有明确互斥和跨 CPU TLB 一致性；上下文切换期间不得开放可重入调度窗口。先验证旧审计中的哪些问题已经修复，避免重复改动。必须增加压力测试和可观察的 shootdown 完成机制。

## 当前已知状态

- FRAME_BUFFER 和 CPU 表已改用 `SpinIrqLock`，这两项旧问题不应重复修复。
- fork 后已调用 `tlb_shootdown()`，但实现没有等待全部目标 CPU acknowledgement。
- 单个全局 `TLB_SHOOTDOWN_PENDING` 会被任一接收 CPU 清除，不能表示所有 CPU 已完成。
- `mprotect`、`munmap` 等页表变更仍缺统一远端 shootdown。
- `PageTableManager::current_mapper()` 可以制造别名可变 mapper。
- FrameInfoArray 的共享访问仍从 `UnsafeCell<&mut [FrameInfo]>` 产生可变引用。
- scheduler 在实际汇编切栈前释放 IRQ lock；`popfq` 与 `ret` 之间也存在窗口。

## 工作范围

1. 设计带 generation/目标 CPU 集合/acknowledgement 的 TLB shootdown 协议；发送方必须在安全点等待所有相关 CPU 完成。
2. 覆盖所有需要远端刷新或 CR3 generation 更新的操作：COW 降权、munmap、mprotect、MAP_FIXED 覆盖、地址空间销毁等。
3. 将页表可变访问收敛到唯一所有权或锁保护，移除可并发创建的 `'static mut` mapper。
4. 修正 FrameInfoArray，使并发原子字段访问不构造别名 `&mut`。
5. 重新设计 scheduler guard 和上下文切换边界，保证 current TCB、栈、CR3、FS base 和 IRQ 状态的切换顺序一致。
6. 消除 `popfq; ret` 可中断窗口，选择可证明安全的恢复序列。
7. 检查地址空间释放时是否还有运行线程，并建立断言或生命周期约束。
8. 保留单核工作能力。

## 主要文件

- `mos_platform/src/smp.rs`
- `mos_platform/src/lib.rs`
- `mos_runtime/src/idt.rs`
- `mos_mem/src/virt.rs`
- `mos_mem/src/ptmanager.rs`
- `mos_mem/src/phys/index.rs`
- `mos_task/src/scheduler.rs`
- `mos_task/src/process.rs`
- `mos_task/src/thread.rs`
- `mos_arch/src/asm.rs`
- `mos_arch/src/percpu.rs`

## 非目标

- 不实现信号或 wait4。
- 不以全局永久关闭中断、禁用 SMP 或退回单核作为最终修复。
- 不只增加延时来掩盖竞争。

## 验收标准

- shootdown 发送方能确认全部目标 CPU 已处理对应 generation。
- 并发页表操作不存在裸别名可变引用。
- context switch 期间不会发生嵌套 schedule 或 current TCB/当前栈不一致。
- 单核与 4 核基础功能不退化。
- 压力测试中无 COW 数据串写、悬空页表、PF/GP/DF 或调度死锁。

## 验证方式

增加并循环执行：

1. 4/8 核 fork+COW，父子并发写不同页并校验内容；
2. 多线程/多进程反复 mmap、mprotect、munmap、MAP_FIXED；
3. 高频 timer interrupt + yield + pipe wakeup 的调度压力；
4. 进程退出和地址空间销毁压力；
5. shootdown generation/ack 统计，确认无超时和遗漏。

至少连续运行数分钟，并把 panic、PF、GP、DF、IPI 超时视为失败。
