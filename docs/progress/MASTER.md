# Current status

- Version under release: `1.1.3`.
- Scope: fold adjacent Grok tool activity, contain long-session scrolling, add Codex/Grok Plan copy and Markdown download, and verify the macOS/webd patch release.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation state

- NexusHub Goal configuration, DTOs, RPC handlers, scheduler and recovery database columns are removed. Goal command names remain unavailable tombstones. Database opening drops `codex_thread_goals` and preserves Probe incidents.
- Codex, Grok and Pi use the shared running indicator. Grok now groups adjacent native tool activity, including Read/List/search and commands; Codex and Pi keep command-only groups. Paired calls/results form one row, and completed groups/rows start closed without resetting user toggles during polling.
- Codex, Grok and Pi assistant replies expose raw-Markdown copy buttons. All provider list rows support double-click rename with Enter/blur save and Escape cancel, subject to existing archive/activity protection.
- Codex and Grok Plan cards provide raw-Markdown copy and title-named `.md` download. Copy feedback is positioned inside its message action bar so long histories cannot expand the outer page.
- Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired. CI contains frontend, backend and macOS Tauri jobs. Release stages are webd Linux, macOS ARM64 and aggregate publishing with seven assets.
- Deployment parameters remain explicit. No private host, domain, credential or real session is stored in Git.

## Fresh checks and acceptance

- WebUI typecheck and 199 unit tests passed after the interaction changes. Chromium and WebKit targeted browser checks passed for long mixed Grok tool runs, scroll containment, Plan copy/download and mobile dark mode.
- Full repository gates, matching CI, seven-asset Release, installed macOS App and authenticated cloud acceptance are pending. The cloud service was observed at `1.0.0` before this patch; upgrade verification must preserve configuration, credentials and provider sessions.
- The previous [v1.1.2 release](https://github.com/lich13/nexushub/releases/tag/v1.1.2) remains historical evidence and does not attest to these new interactions. Linux desktop acceptance remains retired.

Future evidence is appended to this document through normal commits. Do not add another plan, summary or archive Markdown.
