# NexusHub Design System

## Direction

A task-reading workspace aligned with Codex Desktop: neutral light/dark themes, compact navigation, a narrow task list and one readable conversation column. No marketing surface, input composer, permanent inspector or duplicate status cards.

Grok/Pi details refresh every second while running and every two seconds otherwise. Hidden narrow-screen details do not poll. Entering a conversation starts at the latest message; subsequent updates follow only while the reader remains near the bottom. Refresh reloads both the visible list and detail.

## Tokens

Light: canvas `#ffffff`, surface `#f6f6f6`, selected `#ededed`, text `#242424`, muted `#626262`, decorative border `#dedede`, control border `#858585`.
Dark: canvas `#202020`, surface `#191919`, text `#ececec`, muted `#a3a3a3`, border `#3b3b3b`.
System `prefers-color-scheme` and `color-scheme` are the only theme source and react to runtime OS changes. Tokens are declared only on `:root`; components consume them without reverse aliases. Primary commands use neutral foreground/background pairs. Green links and paired success/warning/error tokens identify actual state. Normal text, secondary text and status text require 4.5:1 contrast; essential borders and focus indicators require 3:1. Disabled controls retain readable labels.

## Layout

- Navigation: Codex, Grok Build, Pi, Probe, settings.
- Desktop: 152px navigation, 48px collapsed icon rail, 276px task list, and a message column no wider than 780px. The navigation toggle occupies its own grid track; settings stays at the bottom.
- Body text is 15px, controls are 13-14px, secondary text is at least 12px. Use system fonts and zero letter spacing.
- Codex's explicit archived filter permits reading and restoration. Default views continue to exclude archived, internal and subagent tasks.
- Task actions belong in an anchored menu. Rename is inline; destructive provider deletion uses a scoped confirmation dialog.
- Messages distinguish user input, assistant replies and folded tool activity. Updates for a tool call share one activity row.
- Questions and plans are read-only. Internal collaboration messages and memory citation metadata are not user-visible replies.
- Probe defaults to the event timeline; settings are collected separately. Bark and automatic Goal recovery have independent switches.
- Settings group system/update, maintenance and server-only account/security controls. Execution history expands on demand.
- Cleanup preserves dry-run, expected count, final confirmation, pending and visible failure states.

## Rendering

Use shared theme tokens, Lucide icons and hover labels. Border radius stays at 8px or below. Standard controls have stable dimensions; text wraps without overlaps. Markdown supports code, tables, links and copying. Tool details wrap; code blocks may scroll internally.

At 767px and below, all task workspaces use one mounted list and a list/detail/back transition so search and list scroll survive. Shared top/bottom height variables reserve 54px and 58px. Menus support keyboard entry, arrows, Escape and focus restoration. Native modal dialogs contain focus and disable cancellation during a destructive request. Long titles, paths and errors wrap; only code and tables scroll horizontally.

## Verification

Browser checks cover the server and local development interface. Official macOS acceptance uses the installed Tauri App. Verify no composer or desktop Web service controls, no irrelevant server security on desktop, readable contrast, console health and both cleanup confirmation flows without deleting user data.

Run `pnpm test:browser` for Chromium and WebKit computed-style, contrast, viewport, menu, modal and visibility-driven request regressions. Test both themes at 1440x900, 1280x820, 390x844, 320px and 700/701/767/768px, runtime theme changes and 200 percent zoom. Chromium must retain classic scrollbars; compare document overflow against `documentElement.clientWidth`, which excludes their gutter. Do not impose a body minimum width that exceeds the remaining viewport. References are the public OpenAI Codex repository and official desktop product presentation; NexusHub tokens above are implementation choices, not extracted Desktop source.

Pi uses the same task-reading layout as Grok. Both menus expose 复制线程 ID and report clipboard success/failure. Pi mutation disabled reasons are visible; file deletion describes one JSONL file, while Grok describes one session directory. Five mobile tabs share the available width.

Batch mode uses checkboxes and a visible selected count (maximum 100). Search, status or provider changes clear it; polling never selects new rows. Confirm only allowed preview items. Show per-item results and retain failures for a fresh preview. Restore focus to the invoking action; mobile list/detail and scroll rules still apply. Provider Bark switches are separate, and the unsupported Pi final-failure option explains why it is disabled.
