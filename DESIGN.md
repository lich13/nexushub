# NexusHub Design System

NexusHub is a dense remote-operations console for a cloud Codex app-server. It should feel closer to a Linear/Raycast developer tool than a marketing page.

## Visual Direction

- Keep the outer operations shell dark, compact, and predictable.
- The thread detail surface is a light, conversation-first reading view inspired by Codex Desktop and claudecodeui: quiet background, centered message column, soft borders, comfortable line height, and no log-console feeling.
- Plans, questions, approvals, and tool activity are protocol-specific cells. They must not be rendered as generic assistant text or raw event logs.
- First screen is the usable console: no landing page, no hero section.
- UI copy is operational and short; avoid feature explanations inside the app.

## Color Tokens

```css
--canvas: #07111f;
--surface: #0b1726;
--surface-elevated: #102235;
--hairline: #1d3852;
--primary: #38bdf8;
--primary-hover: #7dd3fc;
--primary-soft: rgba(56, 189, 248, 0.14);
--success: #22c55e;
--warning: #f59e0b;
--danger: #ef4444;
--text: #eaf6ff;
--muted: #8fb3c8;
--chat-canvas: #f5f6f8;
--chat-surface: #ffffff;
--chat-surface-soft: #f8fafc;
--chat-border: #d9e1ea;
--chat-text: #18212f;
--chat-muted: #657386;
```

## Layout

- Desktop: 240px left navigation can collapse to maximize the conversation pane; thread list remains 320px when visible.
- Operations and security pages use compact panels in a two-column grid.
- Mobile under 768px: no sidebar; use top bar, bottom tab nav, full-screen content, and a thread drawer.
- Composer remains stable at the bottom of the conversation area and uses compact Codex-style chips for mode, goal, permissions, model, reasoning, service tier, and cwd.
- Thread detail uses a centered message rail with a maximum readable width. The inspector is secondary and visually quieter than the conversation.

## Components

- Border radius is 6-8px for most controls and panels; avoid large rounded cards.
- Buttons use lucide icons plus short text.
- State chips use sky-blue for running, green for recent/ok, yellow for reply-needed/warning, red for recoverable/danger.
- Permission controls are a single concise menu matching Codex APP choices; do not split network into a checkbox.
- Destructive actions use danger styling and button confirmation; archive cleanup does not require typed text.
- Tool output and job logs use monospace `pre` blocks with wrapping and no horizontal overflow.
- In thread detail, completed tools are grouped or folded by default. Current running/error tools stay visible as compact activity rows with expandable details.
- Proposed Plan cells live in the message history and show the plan body plus `实施计划` / `修改计划` actions only when still current. Historical plans are read-only.
- Questions cells live in the message history and show option buttons, selected state, submit state, and answered history. Old answered questions must not appear as pending.
- Historical chat/tool volume is collapsed behind `显示全部历史`; expanding must not move the composer or create horizontal overflow.

## Mobile Rules

- All inputs and primary buttons should be at least 44px high.
- Thread list becomes a drawer.
- Long logs remain readable with wrapping; do not rely on horizontal scroll.
- Settings pages become a single column.
