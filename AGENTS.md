# Agent Instructions

## Delivery

- Work from the six current Markdown documents and the checked-in contract registry. Keep implementation, tests and acceptance evidence in source or the current status document; do not add release-specific Markdown or backups.
- Preserve user sessions, credentials, configuration and provider-specific behavior. Deployment hosts, domains, private paths and acceptance evidence are explicit runtime inputs and never repository data.
- For code delivery, run the affected Rust, WebUI, Tauri, contract, install and privacy gates before committing. Report real entrypoint evidence separately from fixtures.

## Architecture

- Shared actions flow through `NexusHubUseCases` and the contract registry, then typed WebUI query/domain code and thin Linux/Tauri adapters.
- Codex, Grok and Pi are read-first providers. Native rename and scoped deletion are the only provider mutations. Recheck identity, activity, symlinks and fingerprints before execution.
- The NexusHub Goal feature and automatic recovery chain are retired. Goal RPC names remain only as unavailable contract tombstones. Codex native logs are retained as read-only error-monitor input; no scheduler, recovery retry or Goal database table remains.
- Probe keeps error detection, persistent cursors, dedupe, retention and encrypted Bark delivery. Provider identities and terminal evidence are validated independently. Codex ordinary feedback requests are classified locally only after a verified terminal turn. Native asynchronous questions remain pending across tool acknowledgements and normal completion; all notification entrances use the shared question tracker, identity recheck and persistent per-call delivery claim. Partial answers never change the original dedupe identity. Never replay questions older than the asynchronous enablement baseline.
- Settings use one `更新与维护` surface and the retired `system.status` action cannot execute; capability bootstrap uses `system.capabilities`. File-path Markdown actions must preserve the source path and never invent a web origin.

- The macOS App is the sole UI. Remote management uses an independent `x-api-key` stored in Keychain and a fixed allowlisted Rust HTTP bridge; never send keys from browser fetch, log them or permit redirects. API protocol and connection revisions gate requests. Switching machines cannot reuse previews, selection or cached responses; mutations are never replayed automatically.
- Old web authentication, static assets and EventSource are retired. Preserve API authentication throttling, business audit, Bark encryption and notification state. Database erasure is limited to NexusHub's own retired web credentials; never migrate native provider databases.

## UI and safety

- Use the shared `RunningIndicator` for running Codex, Grok and Pi sessions. Codex, Grok and Pi group adjacent native tool activity at its original position between messages. Never discard historical tool rows; keep chronological call/result pairing and bounded pagination. Both levels use native `<details>` controls. Completed groups are closed by default; active and failed groups remain open. For 1.2.0, AGENTS.md rows and instruction-only groups always start closed; retain explicit user choices by session/activity identity. Plan copy and Markdown download use the same cleaned source as rendering, with safe title-derived filenames. Hide structured memory metadata before parsing, preserve code examples and ordinary prose, and never rewrite native files.
- Batch requests contain 1–100 explicit keys. A changed filter clears selection; polling never selects new rows. Only preview-approved items execute, and per-item failures stay visible.
- Never expose arbitrary shell, public Codex sockets, private deployment values or real session content in tests and packages. Use reserved example values and a GitHub noreply commit identity.
- Keep systemd hardening (`ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true`, `PrivateTmp=true`). Add only exact provider session roots to `ReadWritePaths`; missing Pi storage must not block service startup.

- User-message presentation is separate from assistant Markdown. Preserve user whitespace and literal Markdown. Read native attachment bytes only through `sessions.attachmentRead`, using provider/session/message/attachment identity; never accept client paths or include image bytes in polling responses.
- User message view models recognize complete native question replies and fold AGENTS.md instruction sections before rendering. Keep question/answer text literal, copy only the answer, protect code/quoted examples, and use native message/event identities for disclosure state. No presentation cleanup writes back to native sessions.

## Release matrix

- CI keeps frontend, backend and macOS Tauri checks. Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired.
- Release keeps the macOS ARM64 app/updater and Linux webd stages. The seven assets are the DMG/checksum, updater archive/signature, `latest.json` for `darwin-aarch64`, and webd tarball/checksum.
- Cloud deployment remains manual and explicit through `scripts/deploy-cloud.sh`; production configuration is never committed.

## Required gates

Use the commands in README, plus `python3 scripts/privacy-check.py --git-objects`, `git diff --check`, contract checks and `bash scripts/test-install-script.sh`. Use normal incremental commits after this 1.2.0 change.
