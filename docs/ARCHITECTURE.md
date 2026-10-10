# NexusHub 架构说明（维护者文档）— 1.2.16

本文用于维护者理解模块边界和契约流程；用户能力与下载说明见 [README.md](../README.md)。

## Boundaries

`desktop_embedded_tauri` owns the React UI; `linux_server_api` exposes only the shared core business API. Components call query/domain helpers, typed transport and `NexusHubUseCases`; provider readers and controlled effects remain behind the facade. HTTP handlers and Tauri commands are thin adapters.

| Area | Responsibility |
| --- | --- |
| Codex | Official state/index/rollout/log reads, identity, native rename, archive/restore and preview-bound normal/archived deletion |
| Grok | Native session discovery, branch-aware history, identity, activity checks, rename and scoped deletion |
| Probe | Error detection, provider terminal evidence, cursors, dedupe, redaction, retention and durable Bark delivery |
| Database/config | NexusHub settings, events, deliveries and migration; Codex native data stays outside this schema |
| WebUI | Shared visual contract, provider queries, pure execution-group view models and Plan filename/content preparation |
| Packaging | macOS ARM64 Tauri app/updater and Linux x86_64 headless webd |

NexusHub Goal DTOs, RPC handlers, scheduler, recovery retries and `codex_thread_goals` storage are retired. The old command names are unavailable tombstones only. The database migration rebuilds `probe_error_incidents` without recovery columns and drops the Goal table while retaining incident records.

## Shared changes

1. Update `contracts/nexushub-contract.json` and schema.
2. Implement core DTO/use-case and the Linux/Tauri adapters.
3. Add WebUI query/domain behavior and tests.
4. Run contract, privacy, Rust, WebUI and install gates.

Batch actions use explicit provider keys and return per-item preview/execute results. Files are isolated before deletion and indexes are replaced atomically. Failures retain a recovery path and are not cascaded to unselected records.

`sessions.search` is a shared read-only action. The core searches provider summaries and bounded detail pages using the same visible Markdown cleaner as rendering and export. Its request contains provider, thread/provider scope, an opaque session key, query, cursor and limit; clients cannot submit filesystem paths. Results carry a stable event/block position key. The WebUI scopes requests to the active machine, opens a result in its provider workspace, loads bounded history for Codex/Claude, and uses the shared timeline anchor for centering and highlighting.

## Grok activity

The shared Grok activity snapshot validates array/object registry entries against one process table, native executable identity and process start time. Exited, zombie and reused PIDs cannot prove execution. A bounded cache incrementally consumes complete native primary-turn events, retaining file identity and a prefix anchor to detect replacement/truncation; unread or corrupt state stays unknown. Display requires both a live owner and an unfinished primary turn. Deletion separately rechecks native ownership and file fingerprints before and after quarantine, including idle open sessions.

## Activity timeline

Codex retains completed tool blocks and positions paired results at the native call position. Existing detail/block pagination bounds responses; chat/action compaction preserves separators around tools. WebUI normalizes all providers into adjacent activity groups with stable session/call identities. Completion closes default disclosures, while explicit choices survive polling and remounts. Legacy history summaries remain readable. The timeline rail is outside the document flow and contains only recognized user-message entries; tools, plans, assistant replies and activity groups keep stable anchors for search and focus without creating rail entries. Activity aliases still locate a command inside a closed group without changing its disclosure state. No native file format or database migration is added.

## Visible Markdown

`visibleMarkdown.ts` removes structured memory metadata by source range before rendering and export. CommonMark code positions protect literal examples; a rendering guard also removes metadata elements. Copy buttons, thread previews and Plan filenames/downloads use the same cleaner. Instruction-file recognition feeds native disclosures and execution groups. A bounded in-memory map scoped by provider, session and activity preserves explicit disclosure choices; no database or native session file stores this UI state.

## User message attachments

Provider readers retain a presentation-only user message with a stable native identity, literal request text and attachment descriptors. The Codex reader pairs native image parts with attachment markers; Grok groups chunks within a user-message boundary; Claude groups native text/image parts in the same message. Native files are never rewritten.

The shared frontend user-message view model splits confirmed native question replies and AGENTS.md sections from ordinary literal text. Native question IDs remain internal keys; answer-copy and question disclosure operate independently. All three renderers supply the original event/block identity for legacy messages without a user-message DTO. Instruction and question disclosures reuse the bounded session-scoped choice store. This is a presentation change without a new backend contract or migration.

`sessions.attachmentRead` is an authenticated shared read through `NexusHubUseCases`, with thin HTTP/Tauri adapters. It accepts provider, session key, message ID and attachment ID, re-resolves the source and checks file identity. PNG/JPEG/WebP/GIF previews are limited to 20 MiB each. Images are loaded near the viewport; the frontend cache is bounded and isolated by machine and cleared when the active connection changes. Remote-only references are not fetched. Polling contains descriptors, not image data.

## Files, settings and notifications

File links are parsed from the original Markdown target before browser URL resolution. The shared path component strips line/column suffixes, resolves relative paths against the selected thread workspace, copies paths for remote threads, and delegates Finder reveal to the existing Tauri opener on macOS.

Settings contain `更新与维护` and `远程连接`. `system.status` is a retired tombstone; `system.capabilities` returns API protocol version, host surface and the capability matrix. Updates and cleanup retain their own dry-run records.

Codex final replies are classified locally for explicit user feedback requests after a verified terminal turn. The monitor, Stop Hook and completion path share the `assistant_question` source and persistent delivery claims. Claude completion requires terminal evidence and no pending tools; an empty result falls back to the current turn's last assistant text. Compatibility warnings remain readable and guard mutations; only fatal parse or identity errors suppress notification scanning. Existing provider completion/error rules remain unchanged.

## Probe and notifications

Codex errors are selected from canonical main-task identities. Grok requires a primary native turn ending; Claude requires verified terminal evidence and no pending tool call. Persistent delivery claims prevent duplicate sends after restart; first enable establishes a baseline.

## Release matrix

CI runs frontend, backend and macOS Tauri checks. Release guard checks the tag/version, contract, privacy and matching successful CI through the Actions API. Release packaging has webd Linux, macOS ARM64 and aggregate stages. The server artifact is `nexushub-webd-linux-x86_64.tar.gz`; the updater manifest contains only `darwin-aarch64`, and the webd tarball is a server asset rather than a desktop updater entry.

Future edits update these six documents and use normal Git commits.

Native Codex question parsing lives in `codex/user_input.rs`. It normalizes synchronous `question` and asynchronous `title` fields and string/object options. The async tracker pairs original turn/call/question identities with complete native reply envelopes and handles explicit cancellation or replacement. The webd asynchronous monitor scans independently of list status and shares the sender recheck with Hook, passive and Stop events. Existing settings hold an enablement baseline and atomic per-call delivery records, so first startup skips history, partial answers do not create new identities, and restarts or TTL expiry do not resend confirmed calls. No RPC or database table is added.

## Remote transport and retirement

Desktop `remote.*` commands own connection preferences and Keychain access. They never enter the Linux dispatcher. React sends a command, arguments and connection revision through native IPC; Rust uses a fixed registry allowlist, HTTPS, disabled redirects and a 32 MiB response bound, enough for the existing 20 MiB image limit. On first remote use the protocol is verified; stale responses cannot cross a revised connection. No credential is returned to React.

A revision-scoped QueryClient and workspace lifetime isolate mutations, previews, paging and scroll; attachment keys, title overrides and disclosures include machine identity. The selected machine owns the single update panel and its query/job lifetime. The local target uses Tauri updater; the remote target uses server RPC. Plan saving always stays local. Remote path links never reach Finder.

Server authentication accepts one `x-api-key`, stores only its SHA-256 digest and uses constant-time comparison. Failed authorization is bounded and audited without credentials; valid credentials are not locked out by failed attempts. CLI rotation invalidates the previous Key and revocation denies all business RPCs. Old authentication/security actions and threadEvents are unavailable tombstones.

Opening only the NexusHub database drops administrators, web sessions and Turnstile state, removes retired settings, erases freed pages and checkpoints/truncates WAL. Business audit, jobs, Bark encryption and notification state survive. Config migration removes retired web keys without replacing unrelated settings. Neither native Codex storage nor provider session formats change. Linux packages contain no React assets; the reverse proxy serves RPC and health only.

`plans.save` is a desktop-only native export to Downloads. It validates a bounded Markdown filename/content and uses exclusive file creation; it never traverses the remote HTTP transport.

## Claude Code

`claude` owns recursive native JSONL discovery, bounded incremental file caching, branch-aware timeline parsing and process ownership checks. `claude.*` actions and the `claude_code` provider use the existing core facade, API Key bridge and Tauri adapters. The opaque session key binds relative transcript location and native ID, so duplicate IDs remain independent. Subagent files are not separate primary sessions. Details page by stable event identity; attachments remain lazy and bounded to 20 MiB. Common Claude Code record variants remain readable during discovery and detail loading.

Known 2.x records include user/assistant blocks, title records, tools, queue/history/usage metadata and compaction. Unknown records remain readable; malformed records, incomplete tails or unverified formats disable mutation. Claude running state needs a live identified process, matching start time/session/project and an unfinished turn. Management separately refuses any potentially owning process. Rename appends `custom-title` and verifies persistence; deletion isolates and rechecks one JSONL without changing native indexes, configuration or the workspace.

Claude notification cursors and atomic delivery claims reuse the provider monitor. Only explicit completed or failed turns and unresolved `AskUserQuestion` calls or permission requests bound to a pending primary tool call qualify. Sending rechecks native identity and pending state; first enable and the compatible-reader upgrade establish a baseline without replaying history. New user requests supersede undelivered prior events. Bark transport follows the shared bounded retry policy below, including uncertain network outcomes; previously terminal delivery records are never replayed. Permissions not persisted in the transcript cannot generate notifications. Claude never receives Codex natural-language question inference.

## 1.2.8：定向删除与退役迁移

`selected_codex` 为普通和归档线程生成安全快照；身份、明确终态、DB/索引关联、共享引用和文件指纹全部通过才允许删除。文件先进入私有暂存，数据库事务与索引原子替换配合；失败恢复文件和索引，不删除未选中的子线程。

`cleanup.hiddenDryRun` 返回候选、指纹、允许状态和阻止原因。`cleanup.hiddenExecute` 的 `candidates` 是本次执行边界，`expectedCount` 校验其长度，不与全局隐藏数比较。每项重新检查后用 savepoint 执行；项目级错误记录并继续，公共数据库/索引/存储故障回滚整批。提交前复核暂存文件和数据库完整性。

`system.capabilities.api_version` 为 2，桌面桥接校验版本后才转发业务请求。旧无候选请求不能进入隐藏清理。Pi 只保留 retired tombstone；迁移在解码待投递 JSON 前清除 NexusHub 自有 Pi 数据，使用 secure_delete、VACUUM 和 WAL 截断清理遗留页；其他提供方、密钥和业务记录不变。

窗口生命周期集中于 `desktop_boot`：隐藏创建 → 一次最大化或工作区回退 → Ready 显示。Reopen 只显示、取消最小化和聚焦，不再次设置几何信息。

## 1.2.9：父线程绑定的子智能体读取

`MessageBlock.subagent` 是可选展示信息，原生创建调用与结果通过调用 ID 配对，再由返回的线程 ID 或精确任务路径关联原生父子图。关系有歧义时保持不可用。状态读取子线程回合与未完成工具证据，不由父线程状态或登记存在推断。`ThreadDetail.subagent_updates` 只刷新已加载卡片，避免最新分页遗漏较早委派的状态。

`threads.subagentDetail` 使用主线程 ID、子智能体 ID 和有界历史游标，经共享 use case、Linux RPC 与 Tauri 适配。服务端从当前机器可见主线程验证唯一父链、循环和深度，复核索引、rollout 身份、符号链接与读取前后指纹。子线程保留原始历史，再按页返回；缓存有界且按文件身份失效。客户端不能提交文件路径。

子线程附件通过 `sessions.attachmentRead.rootThreadId` 绑定主线程上下文，复用关系与来源验证、MIME 检查和 20 MiB 限制，不在详情中内嵌图片。能力矩阵新增 `thread_subagents`，缺失视为不支持；API 协议保持 2。主线程列表、通知身份与管理保护不变，无数据库表或原生格式迁移。

React 使用机器、主线程和子线程身份隔离查询、分页及折叠状态；取消或迟到响应不得覆盖新目标。父视图保持挂载，详情面板独立滚动与嵌套返回。共享阅读布局不改变用户纯文本、Plan、路径、记忆过滤和 AGENTS.md 折叠规则。

## 1.2.10：凭据后台串行访问

共享 `CredentialAccess` 用异步信号量限制凭据并发，再通过 `spawn_blocking` 调用原生钥匙串。读取的 15 秒限制覆盖排队和执行；原生操作已经开始后不能取消，因此超时或调用方取消时仍由后台任务持有许可，直到真实结束。尚未开始的阻塞任务可以取消，避免堆积。

保存和移除连接只在等待凭据许可时允许超时；一旦事务开始，等待提交或回滚的明确结果。`ChangeGuard` 随事务进入后台任务，调用方取消不会提前允许切换。读取 Key 及验证能力后再次核对连接修订号、目标和地址；业务响应回写也绑定修订号。错误使用固定文案，不返回原生异常正文；无新增凭据缓存、RPC、数据库字段或 API 协议变更。

## 1.2.11：原生子智能体活动和直属状态

读取器识别 `item_completed → SubAgentActivity`，用原生事件 ID 保留 started、completed、interrupted 和 interacted 的顺序。同次创建调用、结果和开始事件合并，保留稳定搜索锚点；续接产生的新开始时点独立存在。外层 item 完成与子线程终态分开判断。

`SubagentActivity` 增加可选 `eventKind` 与 `eventId`，`ThreadDetail.subagents` 返回可选直属集合、各状态数量和完整性提示。集合在历史分页前计算，按原生子线程 ID 去重，不累计回合或孙级数量。当前状态读取子线程自身回合，复用有界文件缓存与身份失效检查；父文件不变也重新核对直属状态。task_name、已验证路径末段、标题和昵称按统一优先级产生展示名。

现有详情及 `threads.subagentDetail` 同时返回该集合，不增加 RPC、数据库表或协议版本。前端有运行中或正在创建的直属子线程时使用活动轮询频率，旧服务缺少集合时保留阅读并提示升级。历史开始行和当前状态独立刷新，最新页顺序优先且保留已加载历史。

可见文本清理额外识别完整 `external_codex_apps_open_page` 包装及其 page_id JSON；渲染、复制、Plan 导出与搜索过滤，代码和引用示例保留，不改写会话原文件。

## 1.2.12：展示文案精简

只移除共享子智能体组件的通用完整性说明，以及清理工作区的两段重复描述。DTO 的 complete/warning、关联校验、未知状态、逐项跳过及错误结果均保持原有语义；不新增接口或存储变化。

## 1.2.14：完整指令与通知退役迁移

共享 `instructionSegments` 在代码和引用保护范围外识别完整原生 INSTRUCTIONS 标签，结束标签无需独占一行；整个包装成为单一片段，内部标题不改变边界。三个 Provider 的 UserMessage 共用解析和稳定 disclosure 身份。

事件识别继续由现有 Hook、monitor 和原生读取器负责。`notification_delivery` 只投递 Bark，保留已有持久化认领、加密待投递记录、启用基线和结果。投递采用下述有限重试策略，事件识别与 HTTP 投递分离；旧的终态未知或失败记录不回放。

迁移精确删除 Gotify 配置、加密 Token、通道行、队列行、Probe 结果字段与专属测试任务，使用 secure_delete、VACUUM 和 WAL 截断清理遗留页；中断标记保证清理可接续。Bark、API Key、其他设置、任务和原生会话不受影响。`probe.gotifyTest` 仅为不可执行 tombstone；能力、DTO、Tauri、Linux RPC、CLI 与 WebUI 均无执行入口。API 协议仍为 2。

## 1.2.15：安全诊断与持久化重试

`bark` 复用支持 HTTP/2 的连接池并禁止重定向，区分 DNS、连接、TLS、超时、响应读取、HTTP 拒绝和 Bark 业务拒绝。已知永久 APNs 错误优先于外层 HTTP 状态。原始异常只用于进程内分类，输出仅保留安全分类、状态码、耗时、次数与匿名关联标识。

`notification_delivery` 将不可变请求与分段加密入队。现有投递表扩展分段进度、尝试次数、首次时间、下次时间及租约归属，不新增表或公共命令。每分段最多 3 次，等待 15/60 秒（±20%），整体期限 10 分钟；Retry-After 超出剩余期限时终止。认领、确认与事件状态回写使用事务；只有有效租约持有者可确认，过期租约仅恢复未确认分段。

稳定通知 ID 由机器实例、事件与分段生成。Hook 保持短等待，后台每轮最多领取 10 项；重试在来源核验前后读取当前配置，检查事件仍有效、身份未变、开关与目标未变。会话正常追加使用原始前缀与文件身份复核，不因长度变化取消有效事件。成功分段不再投递，结束清除队列正文，界面同步更新同一事件。

迁移保留成功、去重和首次基线，旧失败与终态未知不入新队列。新未知结果允许有限重试，仍可能重复提醒。主动测试使用独立身份和同一队列，最终未受理以失败退出；旧配置导入实现已删除，退休入口明确拒绝。API 协议保持 2。

## 1.2.16 自动历史与存储元数据

`historyScrollCoordinator` 与 `useHistoryScroll` 为主线程、Claude 和子智能体提供统一的首次定位、上滚触发、请求互斥及可见锚点补偿。搜索定位复用同一分页入口；游标不前进或请求失败后停止自动重试。`historyWindows` 在轮询更新时合并已加载页面，保留最早历史游标；来源身份改变或记录截断时使旧窗口失效。

`session_storage` 仅统计调用方已验证归属的文件或目录，使用逻辑字节数和轻量元数据，不读取正文或计算删除指纹。Codex 只统计其 rollout；Claude 包含主 JSONL 和同一项目中经身份确认的同名会话目录；Grok 统计原生专属目录。独立线程、共享数据及项目工作目录不计入，符号链接不跟随。

目录缓存最多 512 项、队列最多 128 项，由两个后台 worker 合并相同目录请求；每次扫描受 10,000 个条目、16 层和 250ms 边界限制，最长 30 秒刷新。普通追加可复用同一文件身份的缓存；截断、替换及归属改变立即失效。未完成、无法确认及部分结果分别返回对应状态，不能用零替代未知。

三个摘要 DTO 增加可选 `storageSize`（bytes、scope、status），经既有列表、详情和 Linux/Tauri 适配层返回。协议保持 2，不新增 RPC、数据库表或客户端路径输入。前端按当前机器的查询缓存显示结果，旧服务缺少字段时显示未知。
