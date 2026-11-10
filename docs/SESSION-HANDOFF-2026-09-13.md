# MOS 会话交接：2026-09-13

本批继续终端/前端基础主线，未创建提交，保留原工作树改动。
完整设计、支持范围和测试入口：[终端模型](learning/terminal-model.md)。

## 已实现

- 新 mos_terminal crate：no_std + alloc，独立字符/颜色/光标/脏格模型，跨 write UTF-8/ANSI。
- 支持基础编辑、擦行/擦屏、延迟换行/滚屏、16/256/RGB 色彩、反色、光标显隐。
- TTY 不再对每次 write 单独校验 UTF-8；OPOST/ONLCR 与串口字节输出一致，winsize 来自实际网格。
- 合成器从终端模型重建背景，再叠加窗口/鼠标；窗口关闭后保留更新过的下层文字。
  所有正常显示写入按 DISPLAY_LOCK → adapter → model 顺序串行化。
- 普通日志在终端启用后仅走串口，删除遮挡 shell 的启动演示日志窗口。
- 修复把相距很远的鼠标/终端损伤合并成巨大矩形导致的过度重绘。32 个独立区域有界排队。

## 支持边界

未实现宽字符/组合字符、滚动区域、备用屏幕、终端查询回包、完整 termios、PTY 和方向键输入。
字体覆盖 Latin/Latin-1，其他 Unicode 在模型内保留但用备用字形显示。
保留旧早期 framebuffer writer；panic 仍依赖已有串口路径，不宣称新的无锁 panic 显示恢复。
窗口用户映射完整释放、共享匿名页、多线程退出、TLB 逐核 ack 仍按旧专项推进。

用户之前那台 8～16 核实机的 sh 无输出没有新实机结果，仍未宣称解决。
可运行 test_shell_probe 和 test_frontend；新增 test_terminal（covered 提示后按 Enter）与
非交互 test_terminal_scroll 供实机检查。

## 产物与已完成验证

镜像：build/image/mos-uefi.img，35,717,120 字节。
SHA256：FB2A801CCDAA8537278B600F15C6DFC795385EBCF1074CFB545B6A4AA2362BC3。

- mos_terminal 9 项 + mos_abi 5 项宿主测试通过。
- 用户程序、kernel check、最新 UEFI release 构建通过。
- 修正损伤策略后，1/4/16 核完整 qemu-ctrl-c runner 全通过，包含窗口/终端/滚屏/目录/
  重复 fork-exec/实际 applet 输出/Ctrl-C/后台隔离。
- verify-terminal.py 在 1/4/16 核均通过：真实色彩像素、窗口未被终端穿透、关闭后文字恢复、
  滚动内容，以及串口 UTF-8/非法字节/关闭 OPOST 的原始 LF。
- 已查看最终 16 核终端细节和全屏滚动截图。截图 build/terminal-detail-16.png、
  build/terminal-qemu-16.png、build/terminal-covered-16.png、build/terminal-scroll-16.png。
- 早期大损伤失败日志保留 build/terminal-before-damage-fix-{4,16}.log，不能当最终结果。
- 最新交互结果 build/terminal-qemu-{1,4,16}-result.log，完整字节日志 build/ctrl-c-qemu-*.log。
- 父仓库与 mos 嵌套仓库 diff --check 通过。

## 下一批建议

方向键/Home/End 输入序列与可等待的终端事件；然后补 poll/select 的就绪通知基础与 PTY
最小契约，为用户态终端提供接口。复杂装饰、字体缩放及完整 VT 后补，继续保留磁盘副线。

最终补充：同一镜像的 1/4 核 qemu-wait4 进程、信号、管道完整回归均 PASS。
证据：build/terminal-baseline-{1,4}-result.log 和 build/wait4-qemu-{1,4}.log。
本批所有启动的 QEMU 均由各 runner 的 finally 清理，未操作其他 VM。
