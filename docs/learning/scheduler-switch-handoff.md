# SMP 切换：离开旧栈之后才能释放调度锁

## 问题与证据

2026-09-11 会话/进程组回归中，单核通过，4 核在旧信号回归期间出现 RIP=0x10
内核页故障。原日志保存于 `build/job-before-switch-fix-4.log`。
故障发生在新身份测试之前，不能仅归因于新 syscall。

检查发现一个确定的竞态：Scheduler::schedule 已将旧任务放回 ready 或 exits 队列，
context_switch 却在调用汇编保存旧现场前 drop 调度锁。另一核可以选择旧任务或
回收它；原核仍在使用旧栈和 old_ctx。drop 还可能恢复中断，让本核在交接中途被抢占。
该竞态与故障现象吻合，但单次故障不能证明所有异常都只有这一个原因。

## 修复与阅读入口

- `mos_arch/src/sync/spin_irq_lock.rs`：guard 将解锁责任交给切换代码，保留原 IRQ
  状态，交接时不解锁、不重新开中断。unsafe 接口要求唯一 guard、恰好一次解锁。
- `mos_arch/src/asm.rs::__switch_context`：保存旧寄存器/FPU/RSP 后切换到新任务栈，
  按 C ABI 对齐栈并调用解锁回调。之后才恢复新任务现场和标志位。临时使用的
  RBX/R12/R13 在保存旧值后使用，回调遵循 callee-saved 约定，最终从新现场恢复。
- `mos_task/src/scheduler.rs`：不再在旧栈上 drop guard。旧任务将来恢复时才按
  自己保存的状态恢复 IRQ。新建任务仍经原 intr_exit 路径进入执行。

解锁回调执行时已不触碰旧栈。另一核此后运行或回收旧任务不会与原核争用其现场。
FPU/TLS 的最终恢复在回调之后，避免回调破坏传入任务的寄存器状态。

## 验证范围

检查生成的汇编确认“保存旧 RSP → 切新栈 → 回调解锁”的顺序。
重新运行单核/4 核完整 QEMU 回归，包含大量 fork/退出、跨任务信号、睡眠、管道和
键盘等待切换；并为此次低概率 SMP 故障安排额外一轮 4 核回归。
测试通过也不表示穷尽全部并发交错；后续不能将解锁重新移动到保存现场之前。

最终结果：修复后单核完整回归及两轮 4 核完整回归均通过，另有 kernel check、
完整 release 构建和 4 个 ABI 宿主测试通过。第一次 4 核成功日志保存于
`build/job-switch-fixed-4-first.log`，第二次为 `build/wait4-qemu-4.log`。
