# 终端字符模型、字节输出与窗口背景恢复（2026-09-13）

## 问题与结果

旧 framebuffer 只丢弃 ANSI 序列，TTY 每次 write 要求完整 UTF-8，窗口覆盖后无法重建
下方字符。现在字符、颜色、光标与解析状态由独立 mos_terminal crate 保存；TTY 接收
任意字节片段，正常画面统一由合成器按字符模型恢复并叠加窗口/鼠标。

早期启动保留无需分配内存的 framebuffer writer。启动输入工作任务时（heap 初始化后）
创建终端模型，启用后普通内核日志仅走串口，不再改变应用的颜色/光标。
内核 panic 保留原有串口输出；直接 framebuffer print 仍可用，但不承诺在显示锁内 panic
时具备可重入/无锁恢复能力。

## 分层

- mos_terminal：no_std + alloc，无硬件和 syscall 依赖；字符格、脏格、光标、UTF-8/ANSI 状态。
- mos_driver/framebuffer：字体/像素适配、早期输出、终端模型与损伤区域；提供背景像素重建。
- mos_display：显示锁 → adapter → 模型的统一锁顺序；恢复终端背景后叠加内核窗口、
  用户窗口、鼠标。TTY 写入更新模型后在同一显示临界区完成合成，避免穿透上层窗口。
- mos_devfs/tty：termios 决定 OPOST|ONLCR 的 LF→CRLF；回显走同一路径。
  TIOCGWINSZ 返回实际字符行列。串口原始字节接口绕过 UART 的 fmt/退格二次加工。

## 当前支持子集

- UTF-8 可跨任意 write 边界；不合法字节在显示中替换，串口仍发送原字节，write 返回接收长度。
- CR、LF（模型内不隐式 CR）、BS（移动，不擦除）、TAB（8 列），延迟自动换行和整屏滚动。
- CSI A/B/C/D/E/F/G/H/f/d、J/K/X、P/@、S/T、s/u；ESC 7/8/D/E/M/c；?25h/?25l。
- SGR 默认/粗亮色/反色、16 色、39/49 复位、分号形式 256 色与 RGB 颜色。
- CSI 参数最多 16 个，数值溢出/过多参数整条忽略；OSC/DCS 等字符串不缓存内容；
  CAN/SUB 或终止符恢复。未知序列不会被当作普通文字绘制。
- 字形支持当前 Noto Latin 与 Latin-1 集合；其他 Unicode 标量保留在模型内，以备用字形绘制。

语义参考：[XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)。
这是明确子集，不等价于完整 xterm/Linux VT；尚无宽字符/组合字符、滚动区域、备用屏幕、
设备查询回包、鼠标报告、完整 termios、字体缩放和 PTY。

## 窗口与损伤

窗口关闭的矩形现在先从字符模型恢复，再叠加仍存活的窗口，不再清黑后丢失终端文字。
屏幕写入与合成器都先取得 DISPLAY_LOCK；正常 TTY 不再直接画 framebuffer 然后重复合成。
去掉启动时遮挡 shell 的演示日志窗口。

新增滚屏回归曾暴露大面积重绘：鼠标在顶部、终端输入光标在底部时，把两个不相交的
脏矩形强制并成一个会导致每个按键重绘大片屏幕。现保留最多 32 个独立矩形，仅合并
相交区域，超限时才保守扩张。初次失败日志保留在 build/terminal-before-damage-fix-{4,16}.log。

## 验证入口

1. `cargo test --locked --offline -p mos_terminal -p mos_abi --lib`：9 项终端测试 + 5 项 ABI。
   包括每一个字节切分点、畸形 UTF-8/CSI、色彩/编辑/滚屏、脏光标和 30000 字节边界序列。
2. `test_terminal`：真实用户态分段写入、色彩/编辑、OPOST、窗口覆盖期间更新文字；
   显示 covered 提示后按 Enter 关闭窗口并检查背景恢复。
3. `test_terminal_scroll`：多于一屏的带色编号行，跨页写入后截图确认上方保留滚动内容。
4. `tools/qemu-ctrl-c.ps1 -Cpus 1|4|8|16`：以上探针、截图、原有 sh/目录/子命令/Ctrl-C 回归。
   `-TerminalOnly` 只跑前端与终端专项。每个实例使用独立 MonitorPort。
5. `tools/verify-terminal.py --cpus N`（Python + Pillow）：检查真实窗口像素保持在顶层、
   关闭后背景恢复、红绿蓝、滚动内容、串口原始 UTF-8/非法字节和关闭 OPOST 的 LF。

## 下一步

- 先接键盘方向/Home/End 等输入序列、终端事件与 PTY 的最小契约，再迁移用户态前端。
- 根据可用程序需求补 poll/select 和阻塞设备就绪通知，不为完整 VT 一次填满所有细节。
- 保留窗口句柄/映射的完整释放、共享匿名映射、多线程退出与 TLB ack 专项。
- 用户原先反馈的 8～16 核实机 sh 无输出仍待实机新镜像结果，本批不把它标为已解决。
