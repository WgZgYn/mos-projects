# nanosleep：可取消的定时等待

## 问题

原实现直接解引用用户 timespec，把纳秒向下取整成毫秒，再登记裸 TCB waker。
它既不能安全处理错误地址，也无法被信号中断。只增加信号唤醒会遗留旧定时器：
线程开始下一次等待后，旧到期事件仍可能把它唤醒。

## 方法与核心代码

- `mos_task/src/signal.rs`：`park_interruptible_with_waker` 将本次 InterruptWait
  包装成 Waker，沿用 pending 检查、Blocked 转换和恢复后的失效协议。
  原管道 API 保留为薄包装，因此两条路径使用同一套信号登记规则。
- `mos_platform/src/timer.rs`：`cancel_timer` 用 Waker 身份删除队列登记。
  如果另一核已取走到期事件，删除找不到它也安全：其 token 已失效，调度器拒绝唤醒。
  定时计算直接使用 PM ticks，向上取整；中间乘法使用 u128，避免秒/纳秒转换溢出。
- `mos_syscall/src/time.rs`：`sys_nanosleep` 先受检复制 16 字节输入，再验证范围。
  每次恢复取消旧登记，检查截止时间，必要时重新等待；可处理信号返回 EINTR，
  rem 非空时受检回写剩余时间并处理 COW/TLB。请求和剩余时间地址可以相同。

不将 nanosleep 加入字节 I/O 的 SA_RESTART 白名单。错误输入返回 EFAULT/EINVAL；
正常完成不触碰 rem。接口背景见 [Linux nanosleep 手册](https://man7.org/linux/man-pages/man2/nanosleep.2.html)。

## 测试

`test_sleep_interrupt` 覆盖空/内核地址、负值及越界纳秒、零时长不写 rem、
handler 的 EINTR、SA_RESTART 下仍中断、req/rem 别名、错误 rem 返回 EFAULT、
跨过旧截止时间的后续睡眠，以及长睡眠中的 SIGKILL。runner 在单核和 4 核执行。

2026-09-10 验证通过：kernel check、独立用户程序 release 构建、完整镜像构建，
以及单核/4 核 QEMU 新增测试和全部既有 runner 回归。
日志：`build/sleep-check.log`、`build/sleep-user-build.log`、
`build/sleep-image-build.log`、`build/wait4-qemu-1.log`、`build/wait4-qemu-4.log`。

测试的后续睡眠检查“不提前返回”；旧事件是否误排队还依赖 token 在调度器锁内
检查的代码不变量，不能仅凭 elapsed-time 断言宣称穷尽所有并发交错。

## 边界与后续

定时精度受平台中断和调度延迟影响，不是实时调度保证。极大时长饱和到 u64 tick
时间线；clock_nanosleep、停止/继续后的 restart_syscall 尚未实现。
TTY 和 FIFO open 等待仍须迁移。下一阶段继续 SIGCHLD、持续键盘输入和前台进程组。
