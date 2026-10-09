# Agent Instructions

## Delivery

- Work from the six current Markdown documents and the checked-in contract registry. Keep implementation, tests and acceptance evidence in source or the current status document; do not add release-specific Markdown or backups.
- Preserve user sessions, credentials, configuration and provider-specific behavior. Deployment hosts, domains, private paths and acceptance evidence are explicit runtime inputs and never repository data.
- For code delivery, run the affected Rust, WebUI, Tauri, contract, install and privacy gates before committing. Report real entrypoint evidence separately from fixtures.

## Architecture

- Shared actions flow through `NexusHubUseCases` and the contract registry, then typed WebUI query/domain code and thin Linux/Tauri adapters.
- Codex, Claude Code and Grok are read-first providers. Native rename and scoped deletion and Codex archive/restore are the only provider mutations. Claude rename only appends a verified `custom-title` to an inactive, known-format transcript; never install Claude, edit its configuration or infer permission waits from generic pending tools. Recheck identity, activity, symlinks and fingerprints before execution.
- The NexusHub Goal feature and automatic recovery chain are retired. Goal RPC names remain only as unavailable contract tombstones. Codex native logs are retained as read-only error-monitor input; no scheduler, recovery retry or Goal database table remains.
- Probe keeps error detection, persistent cursors, dedupe, retention and encrypted Bark delivery. Provider identities and terminal evidence are validated independently. Codex ordinary feedback requests are classified locally only after a verified terminal turn. Native asynchronous questions remain pending across tool acknowledgements and normal completion; all notification entrances use the shared question tracker, identity recheck and persistent per-call delivery claim. Claude completion requires terminal evidence and no pending tools; an empty result falls back to the current turn's last assistant text. Compatibility warnings remain readable and guard mutations; only fatal parse or identity errors suppress notification scanning. Partial answers never change the original dedupe identity. Never replay questions older than the asynchronous enablement baseline.
- Settings use one `更新与维护` surface and the retired `system.status` action cannot execute; capability bootstrap uses `system.capabilities`. File-path Markdown actions must preserve the source path and never invent a web origin.

- The macOS App is the sole UI. Remote management uses an independent `x-api-key` stored in Keychain and a fixed allowlisted Rust HTTP bridge; never send keys from browser fetch, log them or permit redirects. API protocol and connection revisions gate requests. Switching machines cannot reuse previews, selection or cached responses; mutations are never replayed automatically.
- Old web authentication, static assets and EventSource are retired. Preserve API authentication throttling, business audit, Bark encryption and notification state. Database erasure is limited to NexusHub's own retired web credentials, Pi integration and Gotify records; never migrate native provider databases.

Grok display activity and deletion protection are separate: use one native-process snapshot per list, validate PID identity/start time and read primary-turn events through the bounded incremental cache. Ended turns do not spin, but native ownership still blocks deletion. Never fix stale registration by editing native files or terminating user processes.

## UI and safety

- Use the shared `RunningIndicator` for running Codex, Claude Code and Grok sessions. Codex, Claude Code and Grok group adjacent native tool activity at its original position between messages. Never discard historical tool rows; keep chronological call/result pairing and bounded pagination. Both levels use native `<details>` controls. Completed groups are closed by default; active and failed groups remain open. For 1.2.9, AGENTS.md rows and instruction-only groups always start closed; retain explicit user choices by session/activity identity. Plan copy and Markdown download use the same cleaned source as rendering, with safe title-derived filenames. Hide structured memory metadata before parsing, preserve code examples and ordinary prose, and never rewrite native files.
- Timeline rail markers represent only recognized user messages for each Provider; tools, plans, assistant replies and activity groups keep their original anchors for search and focus without becoming rail markers.
- Batch requests contain 1–100 explicit keys. A changed filter clears selection; polling never selects new rows. Only preview-approved items execute, and per-item failures stay visible.
- Never expose arbitrary shell, public Codex sockets, private deployment values or real session content in tests and packages. Use reserved example values and a GitHub noreply commit identity.
- Keep systemd hardening (`ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true`, `PrivateTmp=true`). Add only exact provider session roots to `ReadWritePaths`; missing Claude storage must not block service startup.

- User-message presentation is separate from assistant Markdown. Preserve user whitespace and literal Markdown. Read native attachment bytes only through `sessions.attachmentRead`, using provider/session/message/attachment identity; never accept client paths or include image bytes in polling responses.
- Search is the read-only `sessions.search` action scoped to the selected machine and either the current thread or current Provider. Use cleaned visible content, omit memory metadata, preserve searchable AGENTS.md text, and return stable event/block positions rather than paths. The timeline rail uses the same anchors for messages, plans, attachments and activity groups; history loading preserves scroll height and disclosure choices.
- User message view models recognize complete native question replies and fold AGENTS.md instruction sections before rendering. Keep question/answer text literal, copy only the answer, protect code/quoted examples, and use native message/event identities for disclosure state. No presentation cleanup writes back to native sessions.

## Release matrix

- CI keeps frontend, backend and macOS Tauri checks. Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired.
- Release keeps the macOS ARM64 app/updater and Linux webd stages. The seven assets are the DMG/checksum, updater archive/signature, `latest.json` for `darwin-aarch64`, and webd tarball/checksum.
- Cloud deployment remains manual and explicit through `scripts/deploy-cloud.sh`; production configuration is never committed.

## Required gates

Use the commands in README, plus `python3 scripts/privacy-check.py --git-objects`, `git diff --check`, contract checks and `bash scripts/test-install-script.sh`. Use normal incremental commits after this 1.2.14 change.

## 1.2.8 管理边界

- 线程右键、省略号和键盘菜单共用动作模型；右键目标独立于当前详情。保留焦点返回、边缘避让、双击改名和批量模式。
- Codex 普通与归档线程永久删除必须绑定预览指纹。复核明确终态、原生身份、关联、符号链接及共享引用；不级联删除其他线程。
- 隐藏清理只接受预览返回的候选和指纹。项目级阻止或失败继续下一项，公共数据库、索引或存储错误回滚整批。新出现的隐藏线程不得加入执行。
- Pi 活动契约、读取、管理、界面和监测全部退役，旧命令只能返回 unavailable。先删除自有队列中的 Pi JSON，再移除 Pi 提供方/流/投递/事件及专属设置；保留其原生程序与用户数据。
- API 协议为 2；先升级服务再安装 App。主窗口隐藏创建，仅初始化时适配一次工作区，Ready 和 Reopen 只显示、恢复及聚焦。
- 发布前检查当前树、索引、可达历史和资产。仅发现可达历史敏感值时才按授权重写；平台保留的不可达对象另行记录，不声称已完全擦除。

## 1.2.9 阅读与子智能体边界

- 三个 Provider 共用紧凑阅读基线；复制动作不增加正文行高，用户气泡、工具两层折叠、Plan、搜索与用户指令时间线保持原规则。
- Codex 子智能体只通过原生调用身份、精确任务路径和父子关联读取，不按显示名称猜测。只使用子线程自己的回合证据判断运行状态。
- `threads.subagentDetail` 和带主线程上下文的子线程附件必须校验可见主线程、唯一父链、深度、循环、文件身份与指纹。无任意路径、执行控制或子线程管理能力。
- 子智能体能力缺失时按不支持处理，API 协议保持 2；机器切换关闭面板、隔离缓存并丢弃迟到响应。
- Hook 集成测试必须启动本次 Cargo 构建的 CLI；不得跳过测试或借用已安装的正式服务。浏览器、Tauri 与真实入口验收分别记录。

## 1.2.10 凭据边界

- Keychain 读写统一经过 `CredentialAccess`，不在异步执行线程直接调用阻塞凭据 API。读取超时仍保留在执行工作所持许可，避免并发堆积。
- 保存与移除连接按完整后台事务执行，调用方取消不提前释放连接变更保护；提交或回滚后才允许切换。
- 凭据读取与能力验证后重新验证连接修订号及目标；固定错误消息不含原生异常、Key 或地址。测试不得访问真实钥匙串。

## 1.2.11 子智能体活动与汇总

- 原生 `item_completed` 中的 `SubAgentActivity.kind` 表示历史活动，外层完成不能当作子线程终态。保留开始、完成、中断及交互时点，并与同次创建调用/结果去重。
- 活动、列表和详情共用任务名解析，优先 `task_name`，再使用已验证路径末段；不让昵称或角色覆盖任务名。
- 直属集合在分页前按原生身份去重，当前状态只依据子线程自身回合。父线程未变时也刷新，保留内部完整性标记及各条目的状态与不可用原因；主线程列表不显示子代理数量。
- 汇总和嵌套列表复用只读详情边界、机器隔离及焦点/滚动恢复。API 协议保持 2，不新增 RPC、数据库表或原生写入。
- 完整内部 `external_codex_apps_open_page` JSON 包装从显示、复制、导出和搜索排除；代码与引用示例保留，不产生空用户气泡或时间线标记。

## 1.2.12 界面说明

子智能体汇总和列表不显示通用的统计不完整说明。归档与隐藏线程清理不显示重复描述，保留状态、预览、确认及逐项结果；文案精简不能改变底层完整性判断或管理保护。

## 1.2.14 指令与通知边界

- 完整原生 INSTRUCTIONS 包装允许结束标签与后续环境标签同行；内部标题不能截断折叠。代码、引用示例与不完整结构保持保守处理，原生会话不写回。
- Gotify 已退役；只允许迁移代码和不可执行契约 tombstone 引用旧标识。移除其配置、Token、队列、测试任务和投递字段时，必须保留 Bark 凭据、基线、成功/未知记录及待投递状态。
- Bark 明确临时 HTTP 拒绝最多尝试三次，未知结果不自动重发。接口协议保持 2，不改变 Provider 或事件识别规则。
- 外部通知服务卸载归运维仓库，须明确配置与数据归属；不改共用 Nginx/TLS、其他服务或用户原生会话。
