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
- Old web authentication, static assets and EventSource are retired. Preserve API authentication throttling, business audit, Bark encryption and notification state. Database erasure is limited to NexusHub's own retired web credentials and Pi integration records; never migrate native provider databases.

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

Use the commands in README, plus `python3 scripts/privacy-check.py --git-objects`, `git diff --check`, contract checks and `bash scripts/test-install-script.sh`. Use normal incremental commits after this 1.2.9 change.

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
