# NexusHub 当前状态（维护者）

## 版本与交付阶段

**1.2.13 核心实现及关键验证完成，正在收尾验收与发布。** 正式环境暂为 1.2.12；本版使用普通增量提交，API 协议保持 **2**。CI、Release、正式安装、Gotify 部署与安卓实收在取得证据后更新本文件，不提前记为通过。

## 已完成实现

- 完整 AGENTS.md 原生指令允许结束标签与后续环境标签同行，内部任意级标题不提前截断；正文不再漏入普通用户气泡。保留代码/引用示例、尾随请求、行数字节摘要和手动展开状态。
- Probe 新增独立 Gotify 开关、HTTPS 地址、Application Token、优先级及测试入口。默认优先级 5，普通升级默认关闭；Token 加密保存，读取仅返回配置状态。
- Bark/Gotify 共用现有 Provider 和事件筛选，以稳定事件身份和通道分别持久化认领、结果和首次启用基线。旧记录按 Bark 迁移；一侧失败不重发另一侧成功记录，未知投递结果不自动重试。
- 明确临时拒绝按 60 秒间隔有界重试，发送前复核来源与开关。请求使用 X-Gotify-Key，保留子路径并禁止重定向，不改变现有事件识别规则。
- 共享 probe.gotifyTest、设置与状态 DTO、能力矩阵、Linux/Tauri 和 WebUI 接入完成。旧服务能力缺失时提示升级。
- 运维仓库已准备官方固定镜像、回环监听、独立数据与密钥目录及 WebSocket 反代配置；NexusHub 不恢复网页版，不新增安卓应用。

## 已验证范围

核心受管 Rust workspace **673 项测试通过**，fmt 和 Clippy 通过；前端 **454 项测试**、类型检查及构建通过，执行结果与对应源码快照一致。部署配置本机 **29 项测试通过**。源码及可达 Git 对象隐私扫描 **1,838 项零失败**，契约、安装边界和 diff 检查通过。

上述为代码与隔离夹具证据，不替代正式 App、云端 Gotify 或安卓设备验收。尚未创建本版 tag 或 Release，未声明正式安装、设备实收或清理完成。

## 收尾待办

- Chromium/WebKit 与 Tauri 对应 CI；正式 App 只读检查真实 AGENTS.md 会话。
- 部署 Gotify，配置独立账户及两台机器 Token；验证 HTTPS、WebSocket、独立通知结果和保护边界。
- 安卓官方客户端接收两台机器专用新事件，后台/锁屏及重连由真实设备验证；HTTP 接收与设备实收分别记录。
- 实现 SHA 三项 CI、中文六节发布说明、七项资产与签名/校验、正式 API/App 升级。
- 精确清理本轮测试、构建、下载与部署暂存，保留正式服务、会话、凭据及 monitor。

## 保留边界

保持 Codex、Claude Code、Grok 的只读优先管理、搜索、用户指令时间线、工具折叠、Plan、附件、路径和子智能体详情。Pi 集成与 NexusHub 网页保持退役。

发布资产仍为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json，以及 Linux x86_64 API tarball/checksum，共七项。updater 仅映射 darwin-aarch64。

macOS 包没有 Developer ID 签名或 Apple 公证；既有资源封装校验限制需在本版成品再次核对，Minisign 更新签名不能替代系统代码签名。GitHub 可能保留不可达对象，可达历史扫描不表示平台缓存已全部擦除。私人路径、域名、身份、凭据和真实会话正文不进入文档、测试及发布资产。
