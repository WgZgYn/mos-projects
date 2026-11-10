# 控制终端、前台进程组与 BusyBox Ctrl-C

## 问题

持续键盘任务只能保证无人读取时仍会消费扫描码；如果没有控制终端和前台 PGID，Ctrl-C
仍然只能误发给“当前线程”或整个会话。原 ioctl 还在文件表锁内调用设备并让 TTY 直接
写用户指针，既可能死锁，也会让坏地址在内核态解引用。BusyBox ash 启动作业控制时还会用
`F_DUPFD_CLOEXEC` 保存私有 tty fd，并以 `WUNTRACED` 等待前台组；缺任一项都会退化。

## 方法与核心代码

`mos_task/src/job.rs` 在原 SID/PGID 注册表同一把锁内保存 MOS 当前唯一物理 tty 的关联：
控制会话以 session leader 的 Weak 记录表示，前台组保存为 PGID。fork/exec 通过 SID
自然继承关联；`setsid` 改变 SID 后自然脱离。leader 及其退出记录释放后 Weak 失效，终端
可由后续会话重新取得，不会永久保留旧 PCB。内核为首个 init 会话做一次显式 bootstrap；
常规路径实现 `TIOCSCTTY`、`TIOCGPGRP`、`TIOCSPGRP`、`TIOCGSID`，且只允许同一会话中
存在的组成为前台组。没有凭据和挂断语义前，TIOCSCTTY 的强制窃取明确返回 EPERM。

`mos_syscall/src/fs/ioctl.rs` 先克隆打开文件并释放文件表锁，再按请求导入/导出固定大小的
内核缓冲。设备接口只接触内核缓冲，TCGETS、TCSETS、窗口和 PGID/SID 请求的 NULL、非法页
稳定返回 EFAULT。TCSETS 的输入复制成功后才改变状态；TCGETS 的输出通过受检复制和必要的
跨 CPU TLB shootdown 发布。

`mos_devfs/src/tty.rs` 保存真实 termios，并实现当前 Ctrl-C 所需的行规程：ICRNL、ISIG/
VINTR、ICANON、ECHO/ECHOE/ECHOK、VERASE、VKILL、VEOF，以及 VMIN=0/非零的基础非规范
读取。TCSETS/TCSETSW 同步应用；当前输出是同步的，因此 TCSETSW 无额外 drain；TCSETSF
额外清空未读输入。非零 VTIME 需要定时等待竞态，当前明确返回 ENOSYS。键盘解码将
Ctrl+字母映射成 U+0001..U+001A；输入任务识别 VINTR 后（默认清空未提交输入）快照 tty
前台 PGID，并通过任务层的标准信号发布函数向该组发送 SIGINT，而不依赖 syscall 层。

启动 shell 自身成为前台组；每次 fork/exec 前由父子两侧共同建立子 PGID，父进程将 tty
交给子组，wait 后取回并恢复 termios。BusyBox ash 需要的 F_DUPFD_CLOEXEC 暂按 F_DUPFD
完成原子复制；FD_CLOEXEC 位尚未建模。wait4 接受 WUNTRACED，但停止状态尚未产生，因此
它目前只等待退出，不会返回 stopped/continued 状态。

## 结果与测试

`test_tty_job` 覆盖前台 PGID/SID 查询、受检 ioctl 错误、termios 往返、未实现 VTIME、
非法前台组，以及子进程 setsid 后失去控制终端且不能窃取 tty。`test_wait4` 覆盖
WUNTRACED 不再误报 EINVAL。`tools/qemu-ctrl-c.ps1` 进入真实静态 BusyBox ash，对用户态
忙循环、BusyBox sleep、cat、两进程管道和空提示符注入物理键盘 Ctrl-C；每次都要求 shell
恢复并执行下一条标记命令。另启独立后台 sleep 组，在前台组收到 Ctrl-C 后必须以 Done
正常结束，证明信号未广播到后台组。

2026-09-11 验证：用户程序 release 构建、完整镜像构建、kernel check 和 mos_abi 宿主测试
通过；单核/4 核完整回归均包含 `[tty-job] PASS`，单核/4 核 BusyBox Ctrl-C 专项均通过。
日志为 `build/wait4-qemu-{1,4}.log` 与 `build/ctrl-c-qemu-{1,4}.log`。

## 边界与后续

- 这是单个物理 tty 的控制终端模型，还没有 PTY、多终端索引、SIGHUP/会话 leader 退出挂断。
- 后台组读终端的 SIGTTIN、后台组调用 TIOCSPGRP 的 SIGTTOU，以及 SIGTSTP/SIGCONT、
  WIFSTOPPED/WIFCONTINUED 尚未实现；当前只验证前后台组的 SIGINT 隔离。
- 非规范读取尚无 VTIME 计时，VMIN 大于调用缓冲长度的完整 POSIX 边界待补；OPOST/ONLCR
  和更完整输入标志尚未作用于数据路径。
- F_DUPFD_CLOEXEC 当前尚不在 exec 时关闭；BusyBox 私有 tty fd 会泄漏到 exec 子进程。
- ash 的交互行编辑仍会探测未实现的 poll(7)，cat 会探测 sendfile(40)；两者会回退并不阻塞
  本批 Ctrl-C，但属于后续 runtime syscall 工作。

