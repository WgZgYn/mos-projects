# MOS 会话交接：2026-09-12

本批沿用户最新偏好推进前端基础和实机可诊断性，保留既有未提交改动，未创建提交。
详细实现及明确边界：[窗口生命周期与目录 ABI](learning/frontend-lifecycle-cwd.md)。

## 用户实机反馈

只在当前一台约 8～16 核机器测试：BusyBox sh 中命令立即返回、无明显输出，自带 shell 正常。
用户明确否定“屏幕滚动清空导致看不到结果”的解释。尚无最新镜像实机结果。
已修复的目录 ABI 错误不能独自解释所有命令无输出，不应宣称整个实机问题已解决。

## 本批完成

chdir/getcwd/fchdir、/mem 挂载顺序；shell cd 关闭 fd / HOME 环境；进程镜像资源接口；
窗口 owner 检查、退出/exec 清理、帧引用固定、失败回滚、SHARED 已映射页 fork 保持共享；
有界脏区域与下层到上层的重绘失效传播；framebuffer 滚屏。

实机可运行 `test_frontend` 和 `test_shell_probe`。后者输出各阶段及 wait status；
对应新镜像为 `build/image/mos-uefi.img`。
SHA256：D90E2D88C62B7E06E3BFE47D642DE8E4A7D2DC684356DF2B3FBB77B87F545192。

## 后续主线

1. 收集新镜像实机探针输出；若 QEMU 仍正常，重点比较 CPU/启动路径与实际失败阶段。
   TLB shootdown 无逐核 ack 仍是风险，但不能在缺乏证据时直接定为根因。
2. 独立字符模型与跨 write UTF-8/ANSI 解析，再接绘制适配器和基本色彩/编辑/滚屏。
3. 完整窗口句柄/用户映射生命周期和事件接口，PTY 与用户态终端；装饰/高级颜色细节后补。
4. 回归磁盘、poll/select 与常用 Linux 基础能力路线。

注意：用户窗口与直接 framebuffer TTY 还未统一，覆盖后的 TTY 内容仍缺重绘来源；
destroy 不按可能已复用的旧 VA 盲目 unmap，用户映射保留到地址空间回收。
原有 exec 失败事务性、多线程 exit_group、完整共享匿名映射、通用 munmap/TLB 仍有债务。

## 本批最终验证结果

- 用户程序与 UEFI 镜像 release 构建成功（frontend-user-build.log / frontend-image-build.log）。
- 最新镜像：1/4/8/16 核 qemu-ctrl-c runner 全部 PASS；每份日志都含 frontend 与
  shell-probe PASS，且启动摘要确认对应核数全部 Running。
- 同一镜像：1/4 核 qemu-wait4 完整进程/信号/管道回归 PASS。
- mos_abi 宿主测试 5/5 PASS；父仓库与 mos 嵌套仓库 diff --check 通过。
- 证据：build/frontend-qemu-{1,4,8,16}-result.log，build/ctrl-c-qemu-{1,4,8,16}.log，
  build/frontend-baseline-{1,4}-result.log，build/wait4-qemu-{1,4}.log，
  build/frontend-abi-test.log。未做实机或截图视觉验收。
