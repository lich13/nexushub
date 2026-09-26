# NexusHub Design System

NexusHub follows a quiet, compact reading workspace aligned with Codex Desktop: neutral light/dark themes, a narrow task list and one readable conversation column. There is no composer, permanent inspector or duplicate status card.

## Layout and interaction

- Navigation contains Codex, Grok Build, Pi, Probe and settings.
- Desktop uses a compact navigation rail, a 276px task list and a message column no wider than 780px. At 767px and below, each provider keeps one mounted list/detail/back flow.
- Search, provider changes and status changes clear batch selection. The list shows the selected count and limits batches to 100 explicit keys. Menus support keyboard navigation and restore focus after dialogs.
- Running Codex, Grok and Pi sessions use the same spinner, `aria-label` and reduced-motion behavior. Unknown state remains text.
- Adjacent command calls and results form an `ExecutionGroup`. Native `<details>` provides keyboard, touch and focus behavior. Completed groups start closed; active and failed groups open; non-command tools stay independent.
- Probe settings show notification and error-monitor controls only. Goal recovery controls are removed.

## Visual tokens

Light: canvas `#ffffff`, surface `#f6f6f6`, selected `#ededed`, text `#242424`, muted `#626262`, border `#dedede`.

Dark: canvas `#202020`, surface `#191919`, text `#ececec`, muted `#a3a3a3`, border `#3b3b3b`.

Use system fonts, stable control sizes and at least 4.5:1 text contrast. Focus indicators and essential borders meet 3:1. Long titles and paths wrap; only code and tables scroll horizontally.

## Verification

Browser checks cover both themes, 1440x900, 1280x820 and mobile widths, reduced motion, spinner state, command grouping, keyboard expansion, scroll following and provider empty states. Official macOS acceptance uses the installed Tauri app. Authenticated cloud acceptance uses disposable provider sessions and explicit deployment inputs.
