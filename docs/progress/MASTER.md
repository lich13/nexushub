# NexusHub 当前状态（维护者）

## 版本与交付阶段

**1.2.14 实现及本地门禁完成，等待 CI 与正式更新。** 按最新决定完全移除 Gotify，保留 1.2.13 的 AGENTS.md 完整折叠修复和 Bark。采用普通增量提交，API 协议保持 **2**。

## 当前实现

- 移除 Gotify 设置、测试按钮、发送器、CLI、能力、DTO 和两端执行入口；旧 RPC 仅保留不可执行 tombstone。
- 配置与数据库迁移删除退休通道、Token、队列、投递字段和测试任务，清理遗留页；Bark 凭据、基线、成功及未知记录与重试状态保留。
- 完整 AGENTS.md 包装允许结束标签与后续环境标签同行，内部标题不提前截断；手动展开、纯文本用户气泡和现有阅读功能保持。
- 两台机器已关闭 Gotify 并清除 Application Token；独立容器、网络、未共享镜像、账号与持久化目录已移除。旧路径返回 404，现有 API 和其他服务保持正常。

## 已验证

- 受管 Rust 测试 **668/668**、前端单元测试 **397/397**，Clippy、格式、类型检查和前端构建通过。
- 契约、安装脚本、隐私扫描与 diff 检查通过；可达 Git 对象和当前源文件扫描未发现隐私命中。
- 退役测试覆盖旧配置、加密 Token、通道队列、测试任务、审计、结果字段和 SQLite 遗留页清理，以及 Bark 凭据、去重、基线及其他状态保留。

## 待完成

- 对应提交的三项 CI（含 Chromium/WebKit 和原生 Tauri）、七项正式资产核验、远程 API 更新与 App 安装。
- 正式入口确认旧通道不可用，Bark 和无监听端口的 monitor 保持；清理本轮暂存。

## 保留边界

Codex、Claude Code、Grok 的会话、通知、搜索、用户指令时间线、工具折叠、Plan、附件、路径和子智能体详情继续使用。Pi 集成与 NexusHub 网页保持退役。

发布资产仍为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json，以及 Linux x86_64 API tarball/checksum，共七项；updater 仅映射 darwin-aarch64。

macOS 包没有 Developer ID 签名或 Apple 公证；此前整包 codesign deep/strict 校验未通过，Minisign 更新签名不替代系统代码签名。发布验收需如实记录本版结果。私人路径、域名、凭据和真实会话正文不进入文档、测试和发布资产。
