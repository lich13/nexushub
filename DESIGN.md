# NexusHub 界面设计规范（维护者文档）

本文记录稳定的布局、交互和可访问性约束，不是面向用户的产品说明。

NexusHub 1.2.15 follows a quiet, compact reading workspace aligned with Codex Desktop: neutral light/dark themes, a narrow task list and one readable conversation column. There is no composer or duplicate status card. Codex subagent details open only when requested.

## Layout and interaction

- Navigation contains Codex, Claude Code, Grok Build, Probe and settings.
- Desktop uses a compact navigation rail, a 276px task list and a message column no wider than 780px. At 767px and below, each provider keeps one mounted list/detail/back flow.
- Search, provider changes and status changes clear batch selection. The list shows the selected count and limits batches to 100 explicit keys. Menus support keyboard navigation and restore focus after dialogs.
- Running Codex, Claude Code and Grok sessions use the same spinner, `aria-label` and reduced-motion behavior. Unknown state remains text.
- Codex, Claude Code and Grok pair native calls/results and group adjacent tool rows at the original call position. Text, Plans, questions and history markers end a group. A muted icon row names the provider and shows activity/failure counts even when closed; each expanded row shows its tool name, clipped input preview and state. Groups have a bounded scrolling list and lazily rendered details. Historical tools are never replaced by one global summary. Both levels use native `<details>`: completed groups/rows start closed, active and failed groups/rows open, and user toggle state survives polling and appended events.
- Visible assistant replies in all three providers expose a compact copy button that copies Markdown after structured memory metadata is removed. Codex and Grok plan cards add copy and title-named Markdown download actions. Provider list rows enter inline rename on double-click; Enter or blur saves and Escape cancels, while activity-protected or archived rows remain read-only.
- AGENTS.md tools and body sections default to closed even when running or failed. Instruction-only outer groups also stay closed; mixed groups retain normal state rules. Summaries show the basename, line/byte counts for body sections and activity status without private paths. Native disclosures preserve user choices across polling, appended text, theme changes and mobile navigation. Open file bodies have their own bounded scroll area.
- Remove `oai-mem-citation`, `citation_entries`, `rollout_ids` and standalone `memory_citation` metadata before rendering, copying or downloading. Keep fenced/inline code examples, ordinary memory prose and native session files intact. Plans use the cleaned heading for download names.
- Probe settings show notification and error-monitor controls only. Goal recovery controls are removed. Settings combine update, archive cleanup, hidden-thread cleanup and job history under `更新与维护`; the retired system status panel and polling do not exist.
- Markdown file links copy their source path for remote machines, while local macOS threads reveal a resolved path in Finder and offer a neighboring copy action. Codex ordinary final replies that explicitly wait for user feedback use the existing Bark reply-needed event. Native synchronous/asynchronous questions use the same event while unanswered, with their title, question text and options. An asynchronous acknowledgement and background progress do not dismiss a question; native matched answers update only the corresponding items. Claude completion requires terminal evidence and no pending tools; an empty result falls back to the current turn's last assistant text. Compatibility warnings remain readable and guard mutations; only fatal parse or identity errors suppress notification scanning.
- The reading column has a narrow timeline rail outside the document flow. The rail shows only recognized user messages as compact short lines; assistant replies, plans, attachments and adjacent activity groups retain stable anchors for search and centering without adding rail lines. Mobile uses a compact popup rail. `⌘F` on macOS and `Ctrl+F` elsewhere opens the shared search panel with 当前线程 and 当前 Provider scopes. Results use cleaned visible content, include AGENTS.md text, exclude memory metadata, load older pages when needed, and remain isolated to the selected machine.

## User messages and attachments

User messages use a shared right-aligned blue bubble with normal-weight system text and preserved whitespace. Markdown characters are literal in user requests; assistant Markdown and Plan rendering keep their existing rules. User role labels are omitted. The bubble stays within the reading column and narrows on mobile.

Confirmed native question-reply envelopes become one bubble per answer. A muted one-line question can expand by pointer or keyboard; the answer remains fully visible and literal. The copy button beside each bubble copies only its answer and reports success/failure locally. Native IDs and JSON/XML transport syntax are not displayed.

User AGENTS.md instructions use a compact, initially closed native disclosure at their original position, with only the basename and line/byte counts in the summary. Expanded contents preserve source whitespace and have bounded scrolling. A complete native INSTRUCTIONS envelope includes its internal headings; trailing ordinary requests stay outside. Stable message/section identities preserve toggles across polling, pagination and mobile navigation. Literal examples never become interactive message metadata.

Image attachments sit above the bubble in reserved 96px square thumbnails, with a modal image preview, Escape dismissal and focus restoration. Loading and failed states retain their footprint. File cards show basenames and reuse path controls. Only confirmed native attachment envelopes are removed; unknown text and literal examples are preserved. Image-only messages have no empty bubble.

## Visual tokens

Light: canvas `#ffffff`, surface `#f6f6f6`, selected `#ededed`, text `#242424`, muted `#626262`, border `#dedede`.

Dark: canvas `#202020`, surface `#191919`, text `#ececec`, muted `#a3a3a3`, border `#3b3b3b`.

Use system fonts, stable control sizes and at least 4.5:1 text contrast. Focus indicators and essential borders meet 3:1. Long titles and paths wrap; only code and tables scroll horizontally.

## Verification

Browser checks cover both themes, 1440x900, 1280x820 and mobile widths, reduced motion, spinner state, mixed Grok tool grouping, keyboard expansion, long-session scroll containment, Plan export and provider empty states. The installed Tauri App is the reference desktop surface; remote checks use the same App through the API and explicit deployment inputs.

## Machine scope

Place a compact 本机/腾讯云 selector below the navigation brand, with an icon in the collapsed rail and an accessible native selector. Keep the current target visible; errors never trigger a silent fallback. Disable switching during writes. Settings use 更新与维护 and 远程连接; no login, system-status or security panel remains. Show only the selected machine’s update panel: 本机 App 更新 or 腾讯云服务更新. Use one compact row with 当前版本, 检查更新 and, only for a confirmed new version, 更新至 followed by its version. Normalize the version prefix; the server also offers 清理更新备份. Show 已是最新版本 only as feedback from a successful check, and keep progress/errors beside the controls.

Remote connection inputs are HTTPS 地址 and 管理员 API Key, followed by 验证并保存 and 移除连接. Do not display a saved key. Connection errors and save results stay near the controls. Machine changes clear unexecuted confirmations and selection; stable disclosure identities include the machine. All previous conversation layout, themes, keyboard and reduced-motion rules remain.

Plan export uses native local file saving, including remote sessions. Show success only after UTF-8 content is written; never overwrite an existing download.

Claude Code keeps a fixed navigation entry on both machine targets. Missing native data shows an empty state. Reuse provider lists, reply/Plan actions, attachment previews and stable tool disclosures; paged history prepends without moving the visible anchor. Common record variants remain readable; unknown or incomplete formats and uncertain activity disable management with the native blocker.

## 线程菜单、清理与窗口

线程项右键立即在指针处打开共享操作菜单，不改变当前详情。省略号、Shift+F10 和菜单键使用同一动作列表；Escape、外部点击和列表滚动关闭，菜单在视口边缘自动避让。复制、改名、归档和永久删除都有明确目标，批量模式不混入右键操作。

列表项与外层改名容器均填满可用宽度，保留 min-width: 0；长短标题、运行 spinner、复选框和选中背景的左右边界一致。

永久删除先显示候选范围与保护原因。隐藏清理呈现本次候选的允许状态，结束后显示删除、跳过、失败、剩余数量和逐项原因；全部受保护时显示“无可清理项目”。失败重试需重新预览。

主窗口隐藏创建，初始化时一次最大化，必要时一次工作区回退，然后显示。Dock 重开、最小化恢复和机器切换不重新调整尺寸或位置；不切换 macOS 独立全屏 Space。Pi 不再显示导航或设置项。

## 1.2.9 紧凑阅读与子智能体

保留 780px 阅读列上限，标题栏约 56px，正文 15px、行高 1.65，普通块间距 12px。用户新指令保留额外分隔。正文、工具摘要和子智能体活动文字共用左侧基线，图标使用固定侧栏位置。工具摘要约 30px 高，按读取、搜索、命令和集成生成中文文案；长名称单行省略，展开内容独立滚动。

助手正文不重复显示 Provider 标签。桌面复制按钮位于正文侧边，悬停或键盘聚焦时出现，触屏保持可见；成功和失败反馈不撑高消息。线程 ID 保留复制菜单入口。

Codex 子智能体行独立于工具组，显示任务名称和自身回合状态，缺少已验证关联时显示不可用原因。可用阅读区至少 960px 时详情并排显示，宽约 420px；较窄时覆盖显示并约束键盘焦点。支持 Escape、关闭、焦点返回和嵌套返回；主线程保持挂载与滚动位置。面板独立分页、滚动和折叠，机器、Provider 或主线程变化时关闭并丢弃旧响应。面板只读，不提供子智能体执行或管理动作。

## 1.2.10 远程连接反馈

钥匙串读取在后台串行进行，不阻塞界面。超过 15 秒时在当前操作附近显示错误，不能把超时当作成功或自动切换到本机。保存与移除连接等待已开始的事务完成；操作期间保持切换保护。连接身份变化后，旧读取不得发起业务操作或覆盖新机器结果。

## 1.2.11 原生活动与直属汇总

子智能体历史活动显示“开始工作”“已完成”“已中断”或“已交互”，与当前运行状态分离，刷新不改写过去的文案。任务名优先于随机昵称，活动行、列表和详情标题保持一致。

只在线程详情标题栏下显示紧凑“子智能体”汇总：运行中、已完成及非零异常数量。确认无直属子智能体时不占位；未知和不可用状态保留在对应条目。点击汇总打开右侧列表，运行中的排在前面，点击进入详情并可返回列表；子线程详情使用自己的直属集合。列表、详情、嵌套返回与 Escape 关闭保留阅读位置和焦点。

完整内部页面包装默认不显示，也不进入复制、导出和搜索；纯包装消息不留空气泡或用户指令短线。普通正文、代码与引用示例保留，未确认结构不误删。

## 1.2.12 说明精简

子智能体汇总和列表移除通用完整性说明。清理面板移除标题下的重复描述，状态标记保持右对齐；具体错误和逐项处理结果仍就地展示。

## 1.2.14 完整指令与通知

完整原生 AGENTS.md 包装默认关闭，包括结束标签与后续环境标签同行的记录。摘要按完整片段计算行数和字节数，展开使用受限高度的原文区域，普通请求独立显示。

Probe 仅展示 Bark 设置与既有 Provider、事件筛选。移除退休通道的卡片、测试按钮、加载状态与结果字段，不保留空白占位或额外说明。

## 1.2.15 Bark 状态与测试反馈

同一事件卡片显示最新投递结果，重试成功不新建卡片。“等待重试”显示下次时间；“重试中”显示当前次数；确认成功显示“Bark 已受理”；最终拒绝、结果不明和停止投递分别显示“发送失败”“结果未确认”“已跳过”。错误仅使用安全中文分类，分段通知显示已确认进度。

测试标题统一为“NexusHub 推送测试”，正文分别为“来自本机的测试通知。”或“来自腾讯云的测试通知。”。任务入队不显示发送成功，等待期间保留运行状态；受理和失败结果就地呈现。旧程序名称、配置导入入口和无用诊断字段不进入产品输出。
