# 进程组信号与组等待

## 问题

SID/PGID 身份接口还不足以管理 shell 管道：一个任务通常有多个进程，需要一次
向指定组发信号，再按组收集退出状态，同时保留其他组的进程和退出记录。

## 方法与代码入口

`mos_task::job::signal_targets` 在身份锁下取得组成员的强引用快照，再释放锁。
`mos_syscall::signal::sys_send_signal` 支持 kill(0, sig) 和 kill(-pgid, sig)，
沿用原单目标投递、信号掩码、忽略处置及等待唤醒。目标快照防止查找后引用失效；
不能在持有身份锁时进入信号/等待/调度器链路。signal=0 只探测，不设置 pending。
所有成员先检查单线程支持边界，再开始投递，避免由不支持的多线程目标造成半次操作。

`sys_wait4` 支持 pid=0 和 pid<-1，只匹配调用者自己的子进程。
pid=0 在入口保存调用者的 PGID；按组等待仍保留 WNOHANG、EINTR/SA_RESTART、
受检状态复制与一次回收语义。i32::MIN 单独处理，避免负数取反溢出。

组成员关系会变化。setpgid/setsid 完成后，释放身份锁再唤醒父等待队列；否则父进程
等待的最后一个组成员移走后，可能一直睡到某个无关退出事件。wait4 在队列锁内检查
成员关系并登记等待，成员变化后的通知关闭检查到入睡之间的窗口。

语义参考：[kill](https://man7.org/linux/man-pages/man2/kill.2.html)、
[wait](https://man7.org/linux/man-pages/man2/waitpid.2.html)。

## 测试

`test_group_signal` 创建两个同组子进程和一个异组子进程，检查组信号只终止选定组、
逐个回收对应状态、异组仍存活、组 SIGKILL、signal=0 和极端 PID。
独立场景检查 kill(0) 包含发送者、wait4(0) 选择同组子进程。
改组场景的子进程必须等父进程确认后才退出，从而证明 wait4 的 ECHILD 是成员迁移
触发的重查，而非事后退出碰巧唤醒。原正 PID/TID 和 wait4 回归仍保留。

2026-09-11 验证通过：kernel check、独立用户程序 release 构建、完整内核镜像构建，
单核/4 核 QEMU 新增组信号测试及完整 runner。日志为
`build/group-{check,user-build,image-build}.log` 与 `build/wait4-qemu-{1,4}.log`。

## 当前边界

kill(-1) 全进程广播仍返回 ENOSYS；停止/继续、实时信号、多 UID 权限与多线程进程
仍待补。当前只投递存活线程，只有 zombie 的组不作为可投递目标。
终端前台组、控制终端关联与 ISIG/VINTR 尚未接入，这批不能替代 BusyBox Ctrl-C 验收。
