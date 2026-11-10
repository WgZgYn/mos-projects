# MOS 会话交接：2026-09-11

## 接续约定

用户要求在这里总结，后续编码留到新的对话。完整目标尚未完成；不要把原 goal 标为
complete 或 blocked。新会话按本文和现有工作树重新确认状态，再继续功能开发。
优先编码和功能补齐，必要技术债同步记录；核心代码加解释性注释，每批总结问题、方法、结果、测试。
仓库有大量既有未提交改动，包含用户原有内容；不要 reset、清理、覆盖或自动提交。
`mos` 是嵌套仓库，主仓库 diff 不能替代检查其中的源码。本轮没有创建提交。

总路线：`docs/tasks/ROADMAP.md`，任务 07、12、14、15。

## 已实现并验证的基础

- wait4：退出记录、状态编码、一次回收、WNOHANG、EINTR/SA_RESTART、受检状态复制；
  PID 为独立单调身份，不等同 PCB slab 槽位。现支持正 PID、任意子进程、当前组和负 PGID。
- 信号：Linux x86_64 action/mask/pending、用户 handler/restorer、rt_sigreturn、
  嵌套/掩码、FXSAVE 状态与返回现场验证；syscall/中断用户返回采用 iretq。
  支持正 PID/TID 和进程组信号、SIGPIPE、SIGCHLD 退出通知。
- I/O：read/readv/write/writev 受检复制、页边界/部分进度处理；pipe/TTY 先复制成功再消费。
  pipe、TTY、nanosleep 接入信号中断及过期等待清理；字节 I/O 可 SA_RESTART，nanosleep 不自动重启。
- 输入：常驻内核任务持续解码键盘，TTY 使用 4096 字节缓冲和读等待队列；
  扫描码登记改为先注册再复查，关闭漏唤醒窗口。
- 组身份：setpgid/getpgid/getpgrp/setsid/getsid；fork 继承、exec 保留身份；
  子进程 exec 后父进程改组限制、同会话约束。组长退出不删除其他成员的组身份。
- 组信号/等待：kill(0)/kill(-pgid)、wait4(0)/wait4(-pgid)；
  setpgid/setsid 通知父等待队列，组成员迁走也可结束旧组等待。
- 运行正确性：FS base 在汇编切换路径恢复；调度锁保持到旧现场保存且切入新栈之后才释放，
  修复旧任务可能被另一核提前运行/回收的竞态。

## 当前验收与证据

最新完成批次是组信号与组等待，单核/4 核完整 QEMU runner 均通过。
本次交接再次检查两份日志中 `[group-signal] PASS`，分别在第 664、744 行。
构建/check 日志：`build/group-check.log`、`build/group-user-build.log`、
`build/group-image-build.log`；运行日志：`build/wait4-qemu-1.log`、`build/wait4-qemu-4.log`。
日志会被后续 runner 覆盖，不能把旧 PASS 当新改动已验证。

上一批 SMP 故障及修复保留记录：`build/job-before-switch-fix-4.log`、
`build/job-switch-fixed-4-first.log`。修复后跑过单核和两轮 4 核完整回归；
之后最新组信号批次再次通过双配置。4 个 mos_abi 宿主测试亦曾通过。

## 停止位置：控制终端尚未开始实现

最后一轮只读取了 ioctl、TTY、job 模块和相关 Linux 文档，没有落地控制终端代码。
不要把讨论中的接口设计当作现有实现：

- TIOCSCTTY/TIOCGPGRP/TIOCSPGRP/TIOCGSID 等尚未实现，setsid 尚无真实控制终端可脱离。
- `mos_devfs/src/tty.rs` 的 TCGETS 返回固定配置，TCSETS 不保存真实状态；
  原 TCGETS/TIOCGWINSZ 仍直接写用户指针。
- `mos_syscall/src/fs/ioctl.rs` 持有文件表锁调用设备 ioctl，需先克隆文件引用后释放锁，
  才适合做可能触发缺页准备的受检复制。
- 键盘解码仍为 HandleControl::Ignore，ISIG/VINTR 尚未接到前台组。
- BusyBox sh 的完整前台 Ctrl-C 尚未验收，不应宣称已支持。

## 后续阶段与验收

1. **控制终端及真实输入语义**：设计会话关联、fork/exec 继承、setsid 脱离、首进程退出
   与挂断生命周期；实现前台组 ioctl，受检复制全部相关用户指针。不能仅按当前运行线程
   或 SID 猜测终端归属。termios 需有真实状态与 ISIG/VINTR、canonical/raw 等效果，
   不只保存结构后返回成功。后台读/改前台组的 SIGTTIN/SIGTTOU 边界须明确。
2. **真实 Ctrl-C 场景**：BusyBox sh 的用户态忙循环、sleep、cat 和管道分别运行时注入
   Ctrl-C，确认前台任务结束、shell 存活且恢复提示符、后台组不受影响，单核/4 核通过。
3. **终端显示**：独立字符/光标模型，跨 write 的 UTF-8/ANSI 解析，SGR 颜色、清屏、
   光标移动与滚屏；用户输出和常规内核日志分离，保留 early boot/panic 输出。
   验证彩色 ls、格式化输出和行编辑；不要只让简单 echo 看起来正确。
4. **磁盘与 VFS**：保留现有分层，补稳定 inode/dev/元数据、块设备容量/读写/flush、
   挂载/卸载/同步契约。使用专用测试磁盘镜像先只读挂载，再写入，最终重启读回验证持久性。

## 明确未完成的债务

FIFO open 等待取消、PIPE_BUF 原子写、多线程退出/信号、停止/继续、实时信号、
kill(-1) 广播、SIGCHLD 显式忽略/SA_NOCLDWAIT 自动回收、孤儿收养、CPU 时间记账。
只含 zombie 的组目前不作为可投递目标。TTY 溢出丢弃整字符但未统计。
VFS 共享 File offset 并发串行化、通用文件缺页发布时序、其他 syscall/ioctl 原始指针、
稳定文件元数据和卸载恢复等还需按实际功能依赖补齐。

## 核心阅读入口

- `docs/learning/group-signals-wait.md`
- `docs/learning/session-process-groups.md`
- `docs/learning/scheduler-switch-handoff.md`
- `docs/learning/tty-input-worker.md`
- `docs/learning/interruptible-pipe.md`
- `docs/learning/interruptible-sleep.md`
- `docs/learning/sigchld.md`
- `docs/learning/checked-read.md`、`docs/learning/checked-write.md`

## 本机验证方式

工作目录：`F:\RustroverProjects\mos-projects`，PowerShell。
环境：RUSTUP_HOME=`F:/SDK/rustup`，CARGO_HOME=`F:/SDK/cargo`，
RUSTUP_TOOLCHAIN=`nightly-x86_64-pc-windows-msvc`，CARGO_INCREMENTAL=`0`，CARGO_NET_OFFLINE=`true`。

```powershell
# 用户程序需独立构建：顶层构建器存在吞掉子构建失败的既有风险
# 在 user/user_space 下执行
cargo build --locked --offline --release --bins

# 回到仓库根目录
cargo check --locked --offline -p mos_syscall -p mos --target x86_64-unknown-none
& ./target/release/mos-projects.exe build --target all
cargo test --locked --offline -p mos_abi --lib
& ./tools/qemu-wait4.ps1 -Cpus 1 -TimeoutSeconds 45
& ./tools/qemu-wait4.ps1 -Cpus 4 -MonitorPort 55604 -TimeoutSeconds 45
git diff --check
git -C mos diff --check
```

runner 各自管理自己的 VM，使用镜像快照，不终止其他 QEMU。并发运行时端口必须不同。
首次确认工具/路径是否仍有效；之前 rg 不可用，使用 PowerShell 搜索作为替代。
