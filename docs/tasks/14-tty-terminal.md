# 任务 14：TTY 输入、终端显示与内核日志分离

优先级：交互主线。前台进程组相关部分与任务 07/12 一起推进，不等待 procfs 完成。

## 2026-09-11：控制终端与真实 BusyBox Ctrl-C

- 单物理 tty 现有控制会话和前台 PGID，支持 TIOCSCTTY/TIOCGPGRP/TIOCSPGRP/TIOCGSID；
  setsid 后按 SID 失去控制终端，关联不持有 PCB。
- ioctl 改为文件表锁外执行并只接触内核缓冲；termios/窗口/PGID/SID 用户指针均受检。
- termios 真实保存并执行基础 ICANON/raw、ECHO、ICRNL、VEOF/VERASE/VKILL、ISIG/VINTR；
  TCSETSW/TCSETSF 有同步/清输入语义，非零 VTIME 明确拒绝。
- BusyBox ash 作业控制已启用；忙循环、sleep、cat、管道、空提示符 Ctrl-C 及后台组隔离在
  单核/4 核通过。说明：[控制终端与前台 Ctrl-C](../learning/tty-foreground-ctrl-c.md)。
- SIGTTIN/SIGTTOU、停止/继续、PTY、挂断、完整 VMIN/VTIME 和 OPOST/ANSI 仍待后续。

## 2026-09-11：持续输入与可中断读取

- 独立内核任务持续消费/解码键盘，TTY read 改为共享字节缓冲和可取消等待队列。
- 复制失败保留输入；信号支持 EINTR/SA_RESTART，SIGKILL 可终止阻塞读取。
- 修复扫描码首次检查与等待登记之间的漏唤醒窗口。
- 核心代码与测试说明：[持续输入工作任务](../learning/tty-input-worker.md)。
- 已验证：kernel check、用户程序/完整镜像 release 构建、单核/4 核 QEMU 专项和完整回归。
- 以上 Ctrl-C 主路径已由后续批次完成；显示层和完整 termios/作业控制仍未完成。

## 现状与边界（2026-09-10）

更新：受检读取阶段已添加持久 CharSyncStream 和 UTF-8 待交付字节，read 返回已交付字符
而非强制填满用户缓冲，复制失败后保留数据。以下原始评估中的“每次新建/填满”已被替代；
控制字符、独立输入处理、termios、前台组和 ANSI 仍按本任务推进。

- `mos_devfs/src/tty.rs` 每次 read 新建 CharSyncStream，阻塞读取尝试填满整个用户缓冲区；
  缺少持久输入状态、行缓冲和短读规则。键盘使用 HandleControl::Ignore。
- TCGETS 返回固定值，TCSETS 直接成功但不保存配置；控制字符全零，窗口固定 80×25。
  ioctl 仍直接解引用用户地址，需要在接入新请求时改为受检复制。
- TTY 输出走 mos_driver::print 和串口；`mos_display/src/console.rs` 另有窗口日志渲染。
  应先明确实际输出路由，再统一终端模型，不能只改一个 Console 就宣称 TTY 已完成。
- ANSI 解析、跨 write 的 UTF-8/转义序列状态、滚屏和日志隔离尚未建立。

## 分批实现

### A：Ctrl-C 的依赖链

1. 通用可中断等待：读写、计时和信号竞争只完成一次，取消后移除旧等待项。
2. 持久键盘解码与输入队列：由独立输入处理路径消费 scancode，用户进程不 read 时也能处理 Ctrl-C。
   IRQ 只入队/唤醒，不在硬中断中分配内存、遍历进程或进行复杂信号投递。
3. 任务 12 的会话/进程组、控制终端和前台 PGID；实现并验证 shell 所需的
   setsid/setpgid/getpgrp/getpgid，以及 TIOCGPGRP/TIOCSPGRP 等实际请求。
4. termios 保存与生效：ISIG、VINTR、规范模式行编辑、回显、EOF；Ctrl-C 发给前台组。
   键盘字符处理不直接指定“当前 CPU 的进程”，也不能把 Ctrl-C 简单当普通字节交给 read。
5. SIGCHLD 与 wait4；前台任务退出后 shell 能恢复终端并继续接收命令。

验收：BusyBox sh 启动前台用户态忙循环、sleep、cat 和管道，分别 Ctrl-C 后返回提示符；
shell 本身继续存活。空提示符 Ctrl-C 不退出 shell。每种场景分别在单核/4 核验证。
完整停止/继续作业控制（Ctrl-Z、fg/bg、WUNTRACED）另开阶段，不与第一版 Ctrl-C 混称完成。

### B：TTY 基本语义与安全边界

- TCGETS/TCSETS/TCSETSW/TCSETSF 的受检复制及实际状态；不支持的选项明确返回错误。
- 规范/非规范模式、ECHO、ICRNL/OPOST 等按 applet 需要逐项实现；VMIN/VTIME 有对应测试。
- 输入排队支持多个等待者，不用单个 AtomicWaker 代替完整 TTY 等待模型。
- TIOCGWINSZ 返回实际终端行列；零长 read、短读和 EOF 行为可验证。

### C：终端显示替代用户态日志前端

建立独立的字符单元屏幕模型，记录字符、前景/背景色、光标与滚动区域。framebuffer
和窗口渲染适配同一模型；TTY 写入解析状态跨 syscall 保留，不要求每次 write 都是完整 UTF-8。

首批 ANSI 子集：SGR 复位/基本颜色，CR/LF/BS/TAB，光标移动，擦行/擦屏和滚屏。
后续扩展 256 色/真彩色、更多 VT 序列。未知序列有界跳过，限制参数长度，避免无限缓存。

早期启动和 panic 保留串口/紧急输出。正常内核日志进入独立缓冲或日志视图，用户 shell
输出进入 TTY；内核日志不能改变应用正在使用的颜色和光标。无需一开始实现 PTY 多窗口。

验收：分拆为多个 write 的颜色序列、UTF-8 字符、BusyBox ls 彩色输出、printf 光标控制、
多屏滚动和提示符编辑。解析器用宿主测试，实际 framebuffer 用截图/人工可视检查验证。

## 学习与维护要求

核心注释解释输入解码、行规程、进程组信号和字符渲染的职责边界。
每批交付问题/方法/结果/测试及实际支持矩阵，不把“有 /dev/tty 节点”当作标准 TTY 完成。

## 2026-09-12：前端资源生命周期基础

窗口已接入进程镜像资源清理、owner 检查、创建失败回滚、独立帧引用和脏区域合并。
下层损伤会传递给上层用户窗口。framebuffer 写满改为滚动；ANSI 颜色仍未实现。
同时修复 shell 依赖的 chdir/getcwd/fchdir 和 /mem 挂载方向，并加入 8/16 核诊断场景。
详见 [本批实现与边界](../learning/frontend-lifecycle-cwd.md)。
下一批按独立终端模型 → 显示适配 → PTY/用户态终端推进。

## 2026-09-13：字符模型与 ANSI 子集落地

- 完成硬件无关的 mos_terminal 模型、分段 UTF-8/ANSI、基本/扩展色彩、光标/擦除/滚屏。
- TTY 接收字节流，OPOST/ONLCR 生效，TIOCGWINSZ 返回实际行列；普通日志退出终端画面。
- 正常 TTY 与窗口合成共用显示锁，终端模型能恢复被窗口覆盖的内容。
- 宿主测试、真实像素/串口字节验证和交互回归入口见 [终端模型](../learning/terminal-model.md)。
- 尚未完成宽字符、备用屏幕、滚动区域、PTY、方向键和完整 termios；不能声明完整终端兼容。
