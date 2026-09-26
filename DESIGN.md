# NexusHub Design System

NexusHub 1.1.4 follows a quiet, compact reading workspace aligned with Codex Desktop: neutral light/dark themes, a narrow task list and one readable conversation column. There is no composer, permanent inspector or duplicate status card.

## Layout and interaction

- Navigation contains Codex, Grok Build, Pi, Probe and settings.
- Desktop uses a compact navigation rail, a 276px task list and a message column no wider than 780px. At 767px and below, each provider keeps one mounted list/detail/back flow.
- Search, provider changes and status changes clear batch selection. The list shows the selected count and limits batches to 100 explicit keys. Menus support keyboard navigation and restore focus after dialogs.
- Running Codex, Grok and Pi sessions use the same spinner, `aria-label` and reduced-motion behavior. Unknown state remains text.
- Grok pairs same-call native tool updates and groups every adjacent tool row, including Read/List/search and commands. Pure command groups are labelled “命令执行组”; mixed groups are labelled “工具活动组”. Codex and Pi retain command-only groups. Both levels use native `<details>`: completed groups/rows start closed, active and failed groups/rows open, and user toggle state survives polling and appended events.
- Visible assistant replies in all three providers expose a compact copy button that copies Markdown after structured memory metadata is removed. Codex and Grok plan cards add copy and title-named Markdown download actions. Provider list rows enter inline rename on double-click; Enter or blur saves and Escape cancels, while activity-protected or archived rows remain read-only.
- AGENTS.md tools and body sections default to closed even when running or failed. Instruction-only outer groups also stay closed; mixed groups retain normal state rules. Summaries show the basename, line/byte counts for body sections and activity status without private paths. Native disclosures preserve user choices across polling, appended text, theme changes and mobile navigation. Open file bodies have their own bounded scroll area.
- Remove `oai-mem-citation`, `citation_entries`, `rollout_ids` and standalone `memory_citation` metadata before rendering, copying or downloading. Keep fenced/inline code examples, ordinary memory prose and native session files intact. Plans use the cleaned heading for download names.
- Probe settings show notification and error-monitor controls only. Goal recovery controls are removed.

## Visual tokens

Light: canvas `#ffffff`, surface `#f6f6f6`, selected `#ededed`, text `#242424`, muted `#626262`, border `#dedede`.

Dark: canvas `#202020`, surface `#191919`, text `#ececec`, muted `#a3a3a3`, border `#3b3b3b`.

Use system fonts, stable control sizes and at least 4.5:1 text contrast. Focus indicators and essential borders meet 3:1. Long titles and paths wrap; only code and tables scroll horizontally.

## Verification

Browser checks cover both themes, 1440x900, 1280x820 and mobile widths, reduced motion, spinner state, mixed Grok tool grouping, keyboard expansion, long-session scroll containment, plan export and provider empty states. Official macOS acceptance uses the installed Tauri app. Authenticated cloud acceptance uses disposable provider sessions and explicit deployment inputs.
