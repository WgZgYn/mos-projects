# 会话与进程组身份

## 问题

shell 需要把一个前台任务（可能包含多个管道进程）组织为进程组，再将该组交给终端。
只有 PID 无法表达这个关系。组长退出也不应让仍有成员的组失去身份。

## 方法与核心代码

`mos_task/src/job.rs` 用一个锁串行化 SID/PGID 初始化、fork 继承、exec 标记和
setpgid 权限检查。记录放在 SharedState 中，与退出记录同寿命；全局表仅持 Weak，
不会阻止 PCB 或退出记录释放。fork 不继承 executed 标志，但继承 SID/PGID；
exec 保留身份并标记已执行新映像。

`setsid` 检查调用 PID 是否已被任意组作为 PGID 使用，允许时 SID/PGID 同时设为
自身 PID。`setpgid` 只允许修改自己或自己的子进程；子进程 exec 后拒绝，
跨会话、会话首进程改组及加入不存在的组都拒绝。两个检查与身份更新在同一锁内。

新增 Linux x86_64 syscall：setpgid(109)、getpgrp(111)、setsid(112)、
getpgid(121)、getsid(124)。新增 EPERM 错误码，与原有 EACCES 区分。
规则参见 [setpgid 手册](https://man7.org/linux/man-pages/man2/setpgid.2.html)
和 [setsid 手册](https://man7.org/linux/man-pages/man2/setsid.2.html)。

## 测试

`test_job_identity` 通过管道明确控制父子执行顺序，覆盖继承、父进程给子进程建组、
加入已有组、组长回收后组成员保留身份、创建新会话、禁止修改父进程和跨会话修改、
会话/组首进程限制、非法组、exec 保留身份以及 exec 后父进程 setpgid 返回 EACCES。
每个等待对象使用独立门控，避免用任意一个共享读者消费字节来假定调度顺序。

本轮 4 核旧回归暴露调度锁提前释放的竞态，修复说明见
[切换锁交接](scheduler-switch-handoff.md)。身份功能与该基础修复一起重新验证。

最终验证（2026-09-11）：独立用户程序/完整镜像 release 构建、kernel check 和
4 个 ABI 宿主测试通过；修复后的单核完整回归、连续两轮 4 核完整回归全部通过，
均包含 `[job-identity] PASS`。日志为 `build/job-{check,user-build,image-build}.log`、
`build/job-abi-test.log`、`build/wait4-qemu-{1,4}.log` 和
`build/job-switch-fixed-4-first.log`。

## 边界与后续

本批完成身份接口，尚未连接控制终端；setsid 的脱离控制终端行为须在终端关联引入时
一起实现。kill 的组目标、wait4 的组筛选、tcsetpgrp/tcgetpgrp、ISIG/VINTR 仍待接入。
显式对已退出目标 setpgid 的兼容边界、孤儿进程组、停止/继续及多线程退出仍需补齐。
查询表当前线性扫描，失效弱引用在新登记时清理，后续可按实际规模改成索引。
这不是完整作业控制，也没有完成前台 Ctrl-C 的验收。
