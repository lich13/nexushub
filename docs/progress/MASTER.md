# Current status

- Target version: `1.1.7`; implementation and local validation complete, release acceptance pending.
- Baseline: `1.1.6`, commit `0a6f24f346078d09484d50c5dd63321b8df0d97b`; local main and origin/main matched with a clean working tree.
- Scope: preserve historical tool positions and provide compact, provider-labelled activity groups for Codex, Grok and Pi. No new RPC, database field or native file changes.
- Exactly six current Markdown documents remain. Real sessions and explicit deployment inputs stay outside Git.

## Implementation

Codex no longer discards completed tools before pagination. Paired outputs retain the call's chronological position, including parallel results and pending calls. Chat and action compaction remains, with markers preserving boundaries around activity.

All providers group adjacent tools between replies. Summaries remain visible when complete groups are closed; running/failed groups start open, except instruction-only groups. Rows show tool/input previews and independently disclose output. Stable session/call identities preserve explicit choices across polling, appends, themes and mobile navigation.

## Validation and delivery

Local Rust workspace and standalone Tauri formatting, tests and Clippy passed. WebUI frozen install, typecheck, 245 unit tests, server/Tauri builds and 138 Chromium/WebKit checks passed. The browser checks cover all three providers, 90 historical tools, earlier-page prepend, disclosure persistence, mobile layout, theme changes and bounded scrolling. Contract, installation, privacy and diff checks passed.

Matching CI, seven-asset Release, installed macOS acceptance, authenticated cloud acceptance and scoped cleanup remain pending. Prior release evidence is in Git. Existing attachments, Plan export, memory filtering, AGENTS.md folding, file paths, rename, batch management and Probe remain regression boundaries.
