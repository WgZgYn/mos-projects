     MOS Kernel SMP Safety Audit & Fix Plan                                                                                                                                                             
   
     Context                                                                                                                                                                                            
                                                                                                                                                                                                      
     启用 SMP 多核后，内核崩溃频率增加（#GP、#PF、#DF）。经过全面审查，发现以下问题，按严重性和修复优先级排列。

     ---
     P0 — 必须立即修复（数据竞争 / 双重故障源）

     1. DummyLock 用于跨 CPU 共享数据

     ┌───────────────────────────────────┬──────────────┬────────────────────────────────┐
     │               位置                │     数据     │              风险              │
     ├───────────────────────────────────┼──────────────┼────────────────────────────────┤
     │ mos_driver/src/framebuffer.rs:226 │ FRAME_BUFFER │ 多核同时写线性帧缓冲 → UB      │
     ├───────────────────────────────────┼──────────────┼────────────────────────────────┤
     │ mos_arch/src/cpu_info.rs:6        │ CPUS         │ 多核访问 CPU 注册表 → 数据竞争 │
     └───────────────────────────────────┴──────────────┴────────────────────────────────┘

     修复：
     - FRAME_BUFFER: 将 DummyLock 替换为 PreemptSpinLock 或 SpinIrqLock（_print 中已有 with_disabled_interrupts，加自旋锁即可）
     - CPUS: 将 DummyLock 替换为 SpinIrqLock，或仅在 BSP 启动阶段使用 DummyLock 后冻结访问

     2. Fork CoW 缺少跨 CPU TLB 刷新（mos_mem/src/virt.rs:484-537）

     share_with() 在设置 CoW（清除 WRITABLE 位）后只调用 .flush()（本地 invlpg）。如果另一 CPU 上有同进程线程且 TLB 中缓存了 WRITABLE 条目，该 CPU 可绕过 CoW 直接写入共享帧 → 数据破坏。

     修复：在 share_with() 完成后，发送 IPI 到所有可能持有该进程页表的 CPU，强制其刷新 TLB（invlpg 或 mov cr3）。注意：fork 后父进程先恢复运行，子进程尚未运行，所以只需刷新当前加载了父进程 CR3 的
     CPU。简化方案：fork 完成后，所有活跃 CPU 重新加载 CR3（mov cr3, cr3）。

     3. PageTableManager::current_mapper() 创建别名 &mut（mos_mem/src/ptmanager.rs:92-94）

     每次缺页处理创建新的 OffsetPageTable<'static>，持有 &mut PageTable。两个 CPU 同时处理同一进程的缺页 → 两个 &mut 指向同一页表 → UB。

     修复：让缺页处理持有 MemorySet.addr_space 锁（SpinIrqLock）再进行页表修改，不再独立创建 current_mapper()。或使用 UnsafeCell 包裹页表访问，避免 &mut 别名问题。

     ---
     P1 — 高优先级（潜在崩溃源）

     4. schedule() 在 context_switch 前恢复 IRQ（mos_task/src/scheduler.rs:157-158）

     drop(guard);     // ← 恢复中断
     Scheduler::context_switch(old, new);  // ← 此时中断已开

     若中断在这窗口内触发并调用 schedule()，同一 CPU 上 spin::Mutex 死锁。

     修复：在 context_switch 完成后再 drop guard，或将 guard 传入 context_switch。

     5. WaitQueue 使用 spin::Mutex 而非 SpinIrqLock（mos_task/src/wait_queue.rs:36）

     当前所有调用都在 SCHEDULER 锁内（已关中断），但类型系统不保证。如果任何未关中断的代码路径访问 WaitQueue，中断处理函数若也访问同一队列 → 同核死锁。

     修复：将 WaitQueue 的 spin::Mutex 改为 SpinIrqLock，类型系统强制中断安全。

     6. 上下文切换 popfq → ret 窗口（mos_arch/src/asm.rs:58-60）

     popfq 可能恢复 IF=1，然后 ret 之前若中断到达 → 栈上返回地址被破坏 → iret 到垃圾地址 → #GP / #DF。

     修复：将 popfq 移到 ret 紧前，两者间仅一条指令。或使用 iretq（原子恢复 RFLAGS+CS+RIP）替代 popfq; ret。

     ---
     P2 — 中优先级（正确性增强）

     7. FrameInfoArray::access() 通过 &self 创建 &mut（mos_mem/src/phys/index.rs:46-54）

     多 CPU 同时 access() → 别名 &mut [FrameInfo] → UB（尽管只访问 AtomicU32 字段，在硬件层面安全）。

     修复：将 access() 改为 &self 返回 & 而非 &mut（因为只访问 AtomicU32）。

     8. MemorySet::clear() 无线程计数检查（mos_mem/src/virt.rs:392-396）

     若调用时仍有线程在执行 → 该线程运行在已释放的页表上 → #PF → #DF。

     修复：在 clear() 前添加断言 process.threads.lock().is_empty()。

     9. PreemptSpinLock drop 顺序颠倒（mos_task/src/preempt_session.rs:35-39）

     自旋锁先释放，preempt_count 后减 → 短暂窗口内锁已释放但抢占仍被禁。

     修复：先 dec_preempt_count()，再 drop 自旋锁。

     ---
     P3 — 低优先级（防御性改进）

     10. lazy_static 初始化顺序依赖

     PAGE_TABLE_MANAGER 依赖 PHYSICAL_MEMORY_OFFSET 已初始化，由启动顺序保证但无类型检查。

     修复：在 PAGE_TABLE_MANAGER 初始化前添加 assert!(PHYSICAL_MEMORY_OFFSET.is_initialized())。

     11. Timer 队列 O(n) 扩张唤醒

     TimerQueue::wake_expired() 使用 Vec::swap_remove，在大量定时器时锁持有时长长。

     修复（可选）：使用二叉堆替代线性扫描。

     ---
     优先实施建议

     第一批（1天内，修复已知崩溃源）：
     - P0-1: FRAME_BUFFER + CPUS 的 DummyLock → 真锁
     - P0-2: fork CoW TLB 刷新（所有活跃 CPU 重载 CR3）
     - P1-6: popfq/ret 窗口 → iretq 替代

     第二批（1-2天）：
     - P0-3: current_mapper 别名 → 走 MemorySet 锁
     - P1-4: schedule() 锁守卫生命周期
     - P1-5: WaitQueue → SpinIrqLock

     第三批（按需）：
     - P2-7, P2-8, P2-9
     - P3 改进项
