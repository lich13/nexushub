# NexusHub Design System

## Direction

A task-reading workspace aligned with Codex Desktop: neutral light/dark themes, compact navigation, a narrow task list and one readable conversation column. No marketing surface, input composer, permanent inspector or duplicate status cards.

## Tokens

Light: canvas `#ffffff`, surface `#f6f6f6`, text `#242424`, muted `#717171`, border `#dedede`.
Dark: canvas `#202020`, surface `#191919`, text `#ececec`, muted `#a3a3a3`, border `#3b3b3b`.
Green identifies primary actions; semantic warning/error colors are reserved for actual state. Headers and conversation content use the same theme. No forced white panels inside dark mode.

## Layout

- Navigation: Codex, Grok Build, Probe, settings.
- Desktop: 152px navigation, 276px task list, flexible detail pane with a readable message column.
- Task actions belong in an anchored menu. Rename is inline; destructive Grok deletion uses a scoped confirmation dialog.
- Messages distinguish user input, assistant replies and folded tool activity. Updates for a tool call share one activity row.
- Questions and plans are read-only. Internal collaboration messages and memory citation metadata are not user-visible replies.
- Probe defaults to the event timeline; settings are collected separately. Bark and automatic Goal recovery have independent switches.
- Settings group system/update, maintenance and server-only account/security controls. Execution history expands on demand.
- Cleanup preserves dry-run, expected count, final confirmation, pending and visible failure states.

## Rendering

Use shared theme tokens, Lucide icons and hover labels. Border radius stays at 8px or below. Standard controls have stable dimensions; text wraps without overlaps. Markdown supports code, tables, links and copying. Tool details wrap; code blocks may scroll internally.

Mobile uses the compact navigation and task list/detail transition, with a back action and no horizontal page overflow. Test narrow and wide screens in both themes, including long task titles, code, menus and errors.

## Verification

Browser checks cover the server and local development interface. Official macOS acceptance uses the installed Tauri App. Verify no composer or desktop Web service controls, no irrelevant server security on desktop, readable contrast, console health and both cleanup confirmation flows without deleting user data.
