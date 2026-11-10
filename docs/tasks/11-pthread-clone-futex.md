# 任务 11：基础 pthread——clone、futex 与 TLS

**类别：Linux 兼容功能开发 / 调度与同步开发编码**

## 可直接交给新对话的任务指令

实现静态 musl pthread 所需的最小 Linux 线程 ABI。开始前用任务 09 的 pthread C 探针
确认 musl 实际发出的 clone/futex/robust-list 相关调用；只实现探针和目标场景需要的
flags/operations，未支持组合必须返回正确错误。

## 工作范围

1. 实现 `clone(56)` 所需最小 flags：地址空间、文件表、信号处理、线程组、TLS、
   parent/child tid 等，以当前 musl 调用为准。
2. 区分 PID/TID/线程组；修正 getpid/gettid、exit/exit_group 的线程语义。
3. 实现 futex 最小集合：WAIT、WAKE、PRIVATE_FLAG、超时和被信号中断。
4. 正确处理 CLONE_SETTLS、FS base 和每线程 TLS。
5. 完成 set_tid_address、CLONE_CHILD_CLEARTID 和 futex wake。
6. 根据 musl 探针需求实现最小 `set_robust_list`；未实现时不得假装成功。
7. 线程退出必须释放线程资源，但共享地址空间仅在最后线程退出后释放。
8. 与任务 07 的信号 mask、exit_group 和 wait 语义一致。

## 主要文件

- `mos_abi/src/syscall.rs`
- 可新增 clone/futex ABI 类型和 flags
- `mos_syscall/src/process.rs`
- 可新增 `mos_syscall/src/futex.rs`
- `mos_syscall/src/lib.rs`
- `mos_task/src/process.rs`
- `mos_task/src/thread.rs`
- `mos_task/src/wait_queue.rs`
- `mos_task/src/scheduler.rs`
- `mos_mem/src/virt.rs`
- `mos_arch` 的 FS base/context switch
- `mos_runtime/src/idt.rs`
- 任务 09 创建的 pthread C 探针

## 非目标

- 不实现全部 clone3、PI futex、优先级继承或实时调度。
- 不把 MOS 私有 CreateThread 当成 Linux clone 的直接同义替换。
- 不在没有等待队列的情况下用无限轮询实现 futex。

## 验收标准

- pthread create/join 可重复运行，不泄漏 TID、内核栈或地址空间。
- pthread mutex 和 condvar 基础探针通过。
- TLS 变量在线程间隔离。
- exit_group 终止线程组；单线程 exit 不错误终止其他进程。
- futex timeout、EAGAIN、EINTR 和 wake count 基础语义正确。
- 单核和 4 核压力测试通过。

## 验证方式

静态 musl C 测试至少包含：

1. 多线程 create/join 和返回值；
2. 线程局部变量；
3. mutex 竞争计数；
4. condvar wait/signal/broadcast；
5. futex WAIT 值不匹配、timeout 和 WAKE 数量；
6. 一个线程调用 exit、主线程调用 exit_group；
7. 4 核高频创建/销毁和共享计数压力。

