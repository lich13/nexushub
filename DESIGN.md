# NexusHub Design System

NexusHub 1.2.3 follows a quiet, compact reading workspace aligned with Codex Desktop: neutral light/dark themes, a narrow task list and one readable conversation column. There is no composer, permanent inspector or duplicate status card.

## Layout and interaction

- Navigation contains Codex, Grok Build, Pi, Probe and settings.
- Desktop uses a compact navigation rail, a 276px task list and a message column no wider than 780px. At 767px and below, each provider keeps one mounted list/detail/back flow.
- Search, provider changes and status changes clear batch selection. The list shows the selected count and limits batches to 100 explicit keys. Menus support keyboard navigation and restore focus after dialogs.
- Running Codex, Claude Code, Grok and Pi sessions use the same spinner, `aria-label` and reduced-motion behavior. Unknown state remains text.
- Codex, Claude Code, Grok and Pi pair native calls/results and group adjacent tool rows at the original call position. Text, Plans, questions and history markers end a group. A muted icon row names the provider and shows activity/failure counts even when closed; each expanded row shows its tool name, clipped input preview and state. Groups have a bounded scrolling list and lazily rendered details. Historical tools are never replaced by one global summary. Both levels use native `<details>`: completed groups/rows start closed, active and failed groups/rows open, and user toggle state survives polling and appended events.
- Visible assistant replies in all three providers expose a compact copy button that copies Markdown after structured memory metadata is removed. Codex and Grok plan cards add copy and title-named Markdown download actions. Provider list rows enter inline rename on double-click; Enter or blur saves and Escape cancels, while activity-protected or archived rows remain read-only.
- AGENTS.md tools and body sections default to closed even when running or failed. Instruction-only outer groups also stay closed; mixed groups retain normal state rules. Summaries show the basename, line/byte counts for body sections and activity status without private paths. Native disclosures preserve user choices across polling, appended text, theme changes and mobile navigation. Open file bodies have their own bounded scroll area.
- Remove `oai-mem-citation`, `citation_entries`, `rollout_ids` and standalone `memory_citation` metadata before rendering, copying or downloading. Keep fenced/inline code examples, ordinary memory prose and native session files intact. Plans use the cleaned heading for download names.
- Probe settings show notification and error-monitor controls only. Goal recovery controls are removed. Settings combine update, archive cleanup, hidden-thread cleanup and job history under `更新与维护`; the retired system status panel and polling do not exist.
- Markdown file links copy their source path for remote machines, while local macOS threads reveal a resolved path in Finder and offers a neighboring copy action. Codex ordinary final replies that explicitly wait for user feedback use the existing Bark reply-needed event. Native synchronous/asynchronous questions use the same event while unanswered, with their title, question text and options. Async acceptance and background progress do not dismiss a question; native matched answers update only the corresponding items.

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

Browser checks cover both themes, 1440x900, 1280x820 and mobile widths, reduced motion, spinner state, mixed Grok tool grouping, keyboard expansion, long-session scroll containment, plan export and provider empty states. Official macOS acceptance uses the installed Tauri app. Remote acceptance connects the installed App to the API using disposable provider sessions and explicit deployment inputs.

## Machine scope

Place a compact 本机/腾讯云 selector below the navigation brand, with an icon in the collapsed rail and an accessible native selector. Keep the current target visible; errors never trigger a silent fallback. Disable switching during writes. Settings use 更新与维护 and 远程连接; no login, system-status or security panel remains. Show only the selected machine’s update panel: 本机 App 更新 or 腾讯云服务更新. Use one compact row with 当前版本, 检查更新 and, only for a confirmed new version, 更新至 followed by its version. Normalize the version prefix; the server also offers 清理更新备份. Show 已是最新版本 only as feedback from a successful check, and keep progress/errors beside the controls.

Remote connection inputs are HTTPS 地址 and 管理员 API Key, followed by 验证并保存 and 移除连接. Do not display a saved key. Connection errors and save results stay near the controls. Machine changes clear unexecuted confirmations and selection; stable disclosure identities include the machine. All previous conversation layout, themes, keyboard and reduced-motion rules remain.

Plan export uses native local file saving, including remote sessions. Show success only after UTF-8 content is written; never overwrite an existing download.

Claude Code keeps a fixed navigation entry on both machine targets. Missing native data shows an empty state. Reuse provider lists, reply/Plan actions, attachment previews and stable tool disclosures; paged history prepends without moving the visible anchor. Unknown format or uncertain activity disables management with the native blocker.
