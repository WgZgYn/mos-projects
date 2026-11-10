# 任务 01：凭据处置与本地配置隔离

**类别：配置治理 / 安全治理 / 项目管理（非功能开发）**

## 可直接交给新对话的任务指令

全面处理当前仓库中的本地明文凭据风险。不得在输出、补丁、命令行或日志中显示任何凭据值。先确认文件的跟踪/忽略状态和 Git 历史，再实施最小范围修复。真实令牌轮换需要用户在对应服务完成；不要声称已轮换未实际轮换的令牌。

## 背景

只读检查发现：

- `.codex/config.toml` 含明文访问令牌，目前未跟踪且未被 `.gitignore` 覆盖。
- 根目录 `settings.local.json` 与 `.claude/settings.local.json` 中检测到疑似令牌；它们当前因通配规则被忽略。
- 初步检查未发现这些文件进入 Git 历史，但需重新验证。

## 工作范围

1. 用不输出值的方式确认敏感文件、敏感键和跟踪状态。
2. 检查所有分支和 tag 的 Git 历史是否包含这些凭据或同类令牌模式。
3. 更新 `.gitignore`，明确隔离本地 Codex/Claude 配置和其他只应存在于本机的凭据文件。
4. 如项目确需共享配置，提供不含真实值的示例文件，并让真实值从环境变量或本地忽略文件读取。
5. 向用户列出必须在外部服务中轮换/吊销的凭据类别，但不显示原值。
6. 不删除用户本地配置，除非用户明确授权；优先保留并隔离。

## 主要文件

- `.gitignore`
- `.codex/config.toml`（只读识别敏感键，不复制内容）
- `settings.local.json`（只读）
- `.claude/settings.local.json`（只读）
- 可能新增的无密钥示例文件或安全说明

## 非目标

- 不修改内核或构建逻辑。
- 不提交、推送或上传任何内容。
- 不替用户在第三方服务中创建新令牌。

## 验收标准

- 真实凭据文件不会出现在 `git status` 的未跟踪列表中。
- 示例/文档不含真实令牌或可用凭据。
- Git 历史扫描结果明确；若历史中出现凭据，给出重写历史前的风险和授权需求，不擅自强推。
- 用户得到清晰的外部轮换清单。

## 验证方式

```powershell
git status --short --branch
git check-ignore -v .codex/config.toml settings.local.json .claude/settings.local.json
git ls-files .codex .claude settings.local.json
git log --all -- .codex/config.toml settings.local.json .claude/settings.local.json
```

如使用 secret scanner，只报告文件、规则名和是否命中，必须遮蔽匹配值。
