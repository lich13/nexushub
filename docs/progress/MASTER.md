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
- Full repository gates passed on the release commit. CI run `36246679244` completed the frontend, backend and macOS Tauri jobs successfully. Release run `36246892930` published the seven scoped assets at [v1.1.3](https://github.com/lich13/nexushub/releases/tag/v1.1.3), including the `darwin-aarch64` updater mapping and Linux `webd` archive.
- The signed DMG was installed as macOS App version `1.1.3`; the installed app binary and bundled helper match the release payload. A real Codex Plan was copied as raw Markdown and downloaded through the native App; the saved UTF-8 file matched the clipboard bytes (`5180` bytes, SHA-256 `0ec0dd4b0cb1b11fedeee0299eba9a3de9b129f4c370fdad7771c090a81b6c8e`).
- The authenticated cloud service upgraded from `1.0.0` to `1.1.3`. The release binary hash matches the published webd archive, health is `{"ok":true}`, and systemd retains `ProtectHome=read-only`, `ProtectSystem=full`, `NoNewPrivileges=yes`, `PrivateTmp=yes` and the exact provider write allowlist. Config and environment files, administrator row count, user sessions and credentials were retained; opening the database removed the retired `codex_thread_goals` table. The authenticated Grok workspace visibly loads real tasks, exposes adjacent mixed tool activity as foldable groups, and shows per-reply copy controls.
- The formal macOS acceptance covered Grok mixed-tool folding and long-session scroll containment, Codex Plan copy/download, and release payload privacy. Cloud acceptance covered service health, migration, hardening, and the authenticated Grok entry point. No Pi installation or destructive cloud session mutation was performed.
- Linux desktop acceptance remains retired; current release evidence is the `v1.1.3` tag and the CI run recorded above.

Future evidence is appended to this document through normal commits. Do not add another plan, summary or archive Markdown.
