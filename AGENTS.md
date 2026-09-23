# Agent Instructions

## Context and delivery

- Read `README.md`, `DESIGN.md`, `docs/ARCHITECTURE.md` and `docs/progress/MASTER.md` before non-trivial edits; resume the current status. Keep the six current Markdown documents concise. Do not add per-release Markdown, backups or old-history restoration references.
- Repository: `https://github.com/lich13/nexushub`. Deployment target and domain must be supplied explicitly and remain outside Git. Keep original source repositories and other hosts untouched.
- Preserve macOS ARM64 Tauri, Linux x86_64 Tauri and Linux server WebUI. Windows and Linux arm64 are not current release requirements. Preserve Codex, Grok Build, Pi, Probe and settings.
- Complete matching-SHA CI, the 15 Release assets, exact-tag macOS/cloud installation, official App/authenticated Browser acceptance and Linux `xvfb` smoke. Never substitute fixtures or public HTTP 200 for a real workflow. Record fresh evidence and scoped cleanup in MASTER.

## Architecture

- Shared actions enter `NexusHubUseCases`. Run `node scripts/contract-next-action-checklist.mjs <action-id>` before changes (omit the ID for a new action).
- Change the contract registry `contracts/nexushub-contract.json` first, then core use-case/DTO/plan, WebUI query/domain/runtime, and thin Linux RPC / Tauri command adapters. Keep `contracts/nexushub-contract.schema.json`, `dtoOwner`, request/response DTO catalogs and both language markers synchronized.
- Keep API handlers and tests split by domain. Adapters must not bypass the facade. Tauri thread DTOs stay in their types module. App.tsx remains composition/navigation; components consume hooks and domain helpers, not raw transport or query-cache APIs.
- Host differences come from capabilities, host surface or policy. Shared visual labels and disabled states belong to the visual contract. Server authentication, Turnstile, systemd, Nginx, public endpoint and prune controls never leak into desktop.

## Safety

- Read official Codex DB/index/rollouts/logs without changing its schema. Native rename uses the fixed protocol and verified readback; archived tasks require explicit restoration. No root app-server/socket/bridge dependency in deployment defaults.
- Grok and Pi read native history. Narrow native rename and confirmed local session deletion are the only mutations. Recheck identity, activity, path boundaries, symlinks and fingerprints. Never delete workspaces/worktrees/configuration or start a task.
- Pi activity is conservative without an installed extension. Unknown activity disables writes. Reading does not start Pi, migrate files or write configuration. No automatic provider installation or credential copying.
- Cleanup requires dry-run plus confirmation and count/fingerprint checks. Fixed maintenance jobs retain authorization, CSRF, audit, redaction and failure classification; no arbitrary shell API.
- Codex Probe notifies only confirmed main tasks using the canonical turn's final reply or unresolved question identity. Restricted Goal recovery has its own switch and never starts a turn; unknown/internal identities are suppressed before mutation or notification.
- Retired task execution, historical jobs/followups, manual Goal controls and desktop LAN WebUI stay retired. Preserve historical DB rows without re-execution. Keep only the fixed port-free desktop Hook/monitor helper.
- No public Codex socket, `/nexushub/v1`, `/nexushub/responses` or `/nexushub/metrics`. `/codex-cloud-panel/` and legacy Probe REST routes remain unavailable. Host-root gateway routes belong to other services.
- Preserve users' data, configuration and secrets; do not reset login, bypass Turnstile or modify unrelated hosts to complete acceptance. CLAUDE.md was deliberately removed; do not restore it or introduce repository-local memory.

## Batch, notifications and privacy

- Batches contain 1–100 explicit keys with fresh identity, activity and fingerprint validation; deletion never cascades to unselected children. Preserve transaction/file/index recovery.
- Grok notifications require a primary native turn ending and matching prompt; Pi completion requires the current branch, stopped assistant and no pending tool calls. Never infer final failure from Pi errors, silence or process age.
- Codex log database maintenance is retired in every layer. Preserve its database and read-only error monitor. NexusHub events/dedupe expire independently under `probe.observability.event_retention_days` (default two days).
- Keep private domains, addresses, user paths, credentials and real sessions outside the repository, Git objects and packages. Use reserved test data and GitHub noreply commit identity. The history reset is authorized only for 1.0.0; future changes use normal commits.

## Gates

Use the complete commands in README: root and independent Tauri fmt/test/Clippy, WebUI typecheck/test/server and Tauri builds, browser regressions, install-script/contract guards and git diff --check. Run additional tests only for concrete risks.

Canonical packaging is `package-darwin-arm64.sh`, `package-linux-tauri-x86_64.sh` and `package-webd-linux-x86_64.sh`; `package-linux.sh` is a deprecated compatibility shim. Restore the helper placeholder after packaging. The headless tarball is never an updater asset in `latest.json`.
