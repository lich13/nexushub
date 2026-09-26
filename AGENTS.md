# Agent Instructions

## Delivery

- Work from the six current Markdown documents and the checked-in contract registry. Keep implementation, tests and acceptance evidence in source or the current status document; do not add release-specific Markdown or backups.
- Preserve user sessions, credentials, configuration and provider-specific behavior. Deployment hosts, domains, private paths and acceptance evidence are explicit runtime inputs and never repository data.
- For code delivery, run the affected Rust, WebUI, Tauri, contract, install and privacy gates before committing. Report real entrypoint evidence separately from fixtures.

## Architecture

- Shared actions flow through `NexusHubUseCases` and the contract registry, then typed WebUI query/domain code and thin Linux/Tauri adapters.
- Codex, Grok and Pi are read-first providers. Native rename and scoped deletion are the only provider mutations. Recheck identity, activity, symlinks and fingerprints before execution.
- The NexusHub Goal feature and automatic recovery chain are retired. Goal RPC names remain only as unavailable contract tombstones. Codex native logs are retained as read-only error-monitor input; no scheduler, recovery retry or Goal database table remains.
- Probe keeps error detection, persistent cursors, dedupe, retention and encrypted Bark delivery. Provider identities and terminal evidence are validated independently.

## UI and safety

- Use the shared `RunningIndicator` for running Codex, Grok and Pi sessions. Consecutive command activity is rendered by the pure execution-group view model with native `<details>` controls. Completed groups are closed by default; active and failed groups remain open.
- Batch requests contain 1–100 explicit keys. A changed filter clears selection; polling never selects new rows. Only preview-approved items execute, and per-item failures stay visible.
- Never expose arbitrary shell, public Codex sockets, private deployment values or real session content in tests and packages. Use reserved example values and a GitHub noreply commit identity.
- Keep systemd hardening (`ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true`, `PrivateTmp=true`). Add only exact provider session roots to `ReadWritePaths`; missing Pi storage must not block service startup.

## Release matrix

- CI keeps frontend, backend and macOS Tauri checks. Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired.
- Release keeps the macOS ARM64 app/updater and Linux webd stages. The seven assets are the DMG/checksum, updater archive/signature, `latest.json` for `darwin-aarch64`, and webd tarball/checksum.
- Cloud deployment remains manual and explicit through `scripts/deploy-cloud.sh`; production configuration is never committed.

## Required gates

Use the commands in README, plus `python3 scripts/privacy-check.py --git-objects`, `git diff --check`, contract checks and `bash scripts/test-install-script.sh`. Use normal incremental commits after this 1.1.2 change.
