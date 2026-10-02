# NexusHub 当前状态（维护者）

## 版本

当前版本为 **1.2.6**。它包含会话搜索、Codex 风格时间线、Claude Code Provider、本机与远程机器隔离，以及远程服务更新修复。

- 主分支：`607f73a7c84addc91fe14109689cc9b5408fe562`
- 发布标签：`v1.2.6`
- 发布资产：macOS ARM64 App/updater、`latest.json` 和 Linux x86_64 `webd` 服务包，共七项

## 已交付能力

- Codex、Claude Code、Grok Build、Pi 的会话读取、搜索和时间线定位。
- 当前线程与当前 Provider 搜索，结果按机器隔离，并能加载较早历史定位命中内容。
- 工具调用与结果按原始顺序显示；完成活动默认收起，运行或失败活动展开。
- 用户消息、附件、Plan、文件路径、AGENTS.md 折叠和记忆元数据清理保持一致。
- 本机与远程 API 快速切换；远程 Key 使用 macOS Keychain 保存。
- Bark 完成、失败和需要回复通知，沿用持久化游标与去重。

## 当前边界

- App 只读会话，不提供发送、接管、停止、分叉或审批功能。
- Claude Code 和 Pi 不由 NexusHub 安装或配置；未知格式或活动状态不明确时保持只读。
- Linux 版本只提供 API 和健康检查，不提供网页、登录页或桌面包。
- 远程 Provider 没有原生数据时显示空状态，不将空状态当作原生会话验收。

## 验证

本版本已通过 Rust workspace、Tauri、WebUI、浏览器、契约、安装脚本、隐私和差异检查；frontend、backend、macOS Tauri 三项 CI 均通过。

正式 App 已验证本机与远程切换、更新状态隔离、搜索入口和时间线定位。关闭 App 后 monitor 仍可运行，且不监听额外 TCP 端口。

## 发布说明

GitHub Release 使用中文说明，列出本版本功能、支持平台、七项资产用途和升级提示：[v1.2.6](https://github.com/lich13/nexushub/releases/tag/v1.2.6)。

后续发布需要同步版本文件、六份当前 Markdown、中文 Release Notes、隐私扫描和三项 CI；部署参数、用户会话、凭据及真实验收数据继续放在仓库外。
