# 2026-09-12：窗口生命周期、目录 ABI 与实机诊断

## 本批问题与修改

- 用户反馈：当前一台约 8～16 核实机上，BusyBox sh 执行命令立即返回而无明显输出；
  自带 shell 正常。不是日志滚动造成的现象。尚未取得新镜像实机结果，不宣称根因已全部解决。
- Linux chdir(80) 原先错误接收 fd；现在接收受检路径，旧 fd 接口迁到 fchdir(81)，
  同步重编 user_lib。getcwd(79) 返回包含 NUL 的长度，覆盖小缓冲和非法地址。
- getcwd 按挂载点的可见名字向上遍历；修正 /mem 挂载点与根参数倒置。
  chdir 显式拒绝普通文件。shell 的 cd 关闭临时目录 fd，并传递 HOME=/。
- PCB 增加 executable-image 资源接口；fork 不继承窗口句柄所有权，exec/退出/PCB 销毁
  释放句柄。显式 destroy 同时删除 PCB 中的资源记录，防止重复创建/销毁累积记录。
- 窗口创建先验证大小、坐标、输出指针；映射失败回滚，输出使用受检复制。
  合成器和用户页表分别持有物理帧引用，用户映射消失时不再留下悬空像素指针。
  已映射的 SHARED 页 fork 保持共享，避免窗口父进程的写入被 COW 分离到另一块帧。
- dirty/destroy 检查 owner PID；窗口脏区域裁剪并合并，避免恶意尺寸溢出和无限排队。
  移除窗口使区域失效；下层重绘的损伤传给用户窗口层，防止覆盖仍存活的窗口。
- framebuffer 写满改为滚动，不再整屏清空。这是独立体验改进，不是实机故障解释。

ABI 对照：[chdir/fchdir](https://man7.org/linux/man-pages/man2/chdir.2.html)、
[getcwd](https://man7.org/linux/man-pages/man3/getcwd.3.html)。

## 测试入口

- `test_frontend`：目录/挂载点/错误码/fork 工作目录隔离，窗口创建、跨进程拒绝、
  子进程带窗口退出、父进程 fork 后继续写像素、重复销毁和尺寸/指针边界。
- `test_shell_probe`：可直接在实机 MOS shell 输入。执行 BusyBox sh -c，检查目录及
  命令替换、ls/date、连续 16 次外部 applet；输出 wait status 和 PASS/FAIL。
  非零退出码 11～15 对应目录阶段，16=ls，17=date，18=重复 exec，19=启动 sh 失败。
  它补充交互测试，不能替代真实交互输入路径。
- `tools/qemu-ctrl-c.ps1`：支持 1/4/8/16 核，先跑以上两个探针，再进入交互 sh，
  检查 echo、cd/pwd、ls/date 的新输出，最后回归 Ctrl-C 与后台隔离。
  检查前记录日志偏移，并剥离 ANSI 后匹配，避免旧输出/输入回显造成假阳性。

## 保留的边界与下一步

- destroy 释放窗口及合成器引用，用户像素映射仍归地址空间所有；不盲目按旧 VA
  unmap，避免该 VA 被应用重新映射后误删别的内存。完整 munmap/重映射生命周期仍需专项。
- 现有 exec 仍先清旧镜像再加载，新镜像加载失败的事务性尚未完成。
- 现有 exit_group 的多线程语义尚不完整，本批沿用当前单用户线程进程边界。
- 用户窗口和直接 framebuffer TTY 尚未统一：移除窗口会重绘内核窗口/桌面及上层窗口，
  不能重建曾被覆盖的直接 TTY 历史像素。需要字符模型作为重绘来源。
- ANSI 仍只是跳过转义序列，不宣称支持颜色/光标/完整 UTF-8 流。下一批建立独立
  终端模型和解析器，先接现有显示后端，再以 PTY/事件接口迁移到用户态终端。
- 16 核 QEMU 通过也不等价于实机通过。TLB shootdown 仍无逐核完成确认，属于后续
  多核排查重点，但目前没有证据将实机无输出直接归因为它。

## 最终验证（2026-09-12）

最新 UEFI 镜像 release 构建成功。1/4/8/16 核前端探针、shell-probe 和交互 Ctrl-C runner
全部 PASS；1/4 核完整 wait4/信号/管道回归 PASS；mos_abi 5/5 宿主测试通过。
日志入口见 [交接记录](../SESSION-HANDOFF-2026-09-12.md)。未做实机和截图视觉验收。
