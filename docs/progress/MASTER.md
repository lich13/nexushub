# NexusHub 当前状态（维护者）

## 版本

当前版本为 **1.2.7**。本版包含仅显示用户指令的时间线短线、Claude Code 记录读取兼容和 Bark 完成通知修复。

- 主分支：`main`
- 发布标签：`v1.2.7`
- 发布资产：macOS ARM64 App/updater、`latest.json` 和 Linux x86_64 `webd` 服务包，共七项

## 当前能力

- Codex、Claude Code、Grok Build、Pi 的会话读取、搜索和时间线定位。
- 当前线程与当前 Provider 搜索，结果按机器隔离，并能加载较早历史定位命中内容。
- 工具调用与结果按原始顺序显示；完成活动默认收起，运行或失败活动展开。
- 四个 Provider 的时间线短线仅显示识别到的用户指令；工具、计划、助手和活动仍保留原始锚点并可搜索。
- 用户消息、附件、Plan、文件路径、AGENTS.md 折叠和记忆元数据清理保持一致。
- Claude Code 兼容常见记录读取；未知或不完整记录继续保持只读。
- 本机与远程 API 快速切换；远程 Key 使用 macOS Keychain 保存。
- Bark 完成、失败和需要回复通知；Claude 的空结果回退到当前回合回复，工具执行中和取消不误报完成。

## 当前边界

- App 只读会话，不提供发送、接管、停止、分叉或审批功能。
- Claude Code 和 Pi 不由 NexusHub 安装或配置；未知格式或活动状态不明确时保持只读。
- Linux 版本只提供 API 和健康检查，不提供网页、登录页或桌面包。
- 远程 Provider 没有原生数据时显示空状态，不将空状态视为已启用的原生会话。

## 验证

发布门槛包括 Rust workspace、Tauri、WebUI、浏览器、契约、安装脚本、隐私和差异检查；frontend、backend、macOS Tauri 三项 CI 需通过。

本地门禁已通过：根 workspace 598 项测试、Tauri 71 项测试及各自 fmt/Clippy；WebUI 271 项单元测试、204 项 Chromium/WebKit 回归、类型检查和构建。安装、契约、隐私与差异检查通过。

1.2.7 已完成正式安装、远程 API 部署和设备 Bark 验收：本机真实 Claude 回合完成通知已送达；远程机器无 Claude 原生会话时显示空状态。关闭 App 后本机 monitor 继续运行且无监听端口。

## 发布说明

GitHub Release 使用中文说明，列出本版本三项修复、支持平台、七项资产用途和升级提示：`v1.2.7`。

后续发布需要同步版本文件、六份当前 Markdown、中文 Release Notes、隐私扫描和三项 CI；部署参数、用户会话和凭据继续放在仓库外。
