# Agent Instructions

NexusHub is a Rust + React operations console built from the codex-cloud-panel base. Treat this repository as the new product surface and keep the original source repositories untouched unless the user explicitly asks otherwise.

## Required Context

- Read `README.md`, `DESIGN.md`, and `docs/progress/MASTER.md` before non-trivial edits.
- If `docs/progress/MASTER.md` exists, resume from its current phase instead of restarting the plan.
- Current upstream is `https://github.com/lich13/nexushub`.
- The production Linux deployment is `https://661313.xyz/nexushub/` on `43.155.235.227`; the legacy `/codex-cloud-panel/` public path has been retired and should return `404`.

## Safety Boundaries

- Preserve the Codex local-state boundary. The WebUI must never expose Codex control sockets, `/v1`, `/responses`, or metrics publicly.
- Keep Codex state reads based on the official Codex state DB, `session_index.jsonl`, rollout files, and `logs_2.sqlite`. Do not mutate official Codex database schemas.
- Do not require `codex-app-server-root.service`, `app_server_socket`, bridge settings, or app-server reloads in deploy defaults. New runtime paths must use local Codex state and controlled jobs instead.
- Do not add arbitrary shell execution to the WebUI. Maintenance actions must be fixed jobs with authorization, CSRF protection, audit records, output redaction, and failure classification.
- Archive and hidden-thread deletion require dry-run visibility plus button confirmation.
- Do not add a WebUI network-access checkbox. Generated sandbox policies default network access to enabled.
- Claude Code provider work is read-only in V1: project/session/settings discovery only. Do not write `~/.claude` or launch/resume/send/stop Claude sessions unless a later task explicitly adds that feature with tests.
- Probe is now an internal NexusHub replacement path for the cloud `codex-sentinel-server` runtime. Keep it observable and maintenance oriented: no hidden desktop control, no automatic recovery/reply, no arbitrary shell, and no direct destructive deletion endpoint outside the existing dry-run plus confirmation model.

## Architecture Boundaries

- Shared Goal, thread, settings, Probe, security, cleanup, upload, job, and update contracts must enter core through `nexushub_core::services::use_cases::NexusHubUseCases`.
- Linux HTTP handlers and macOS Tauri commands/services may execute host DB, job, filesystem, updater, and Codex-state effects from core plans, but must not bypass the facade by calling lower-level `*_with_capability` helpers for shared business contracts.
- Linux HTTP API entry code must stay split by domain under `crates/nexushubd/src/api/`; do not re-inline auth, Probe, security, cleanup, Goal, job, system/update, or upload handlers back into `api.rs`.
- Facade entry files must not become test warehouses again. Keep large API/Codex/Tauri source scans and integration fixtures in domain test modules such as `api/integration_tests.rs`, `api/test_support.rs`, `codex/tests.rs`, `codex/test_support.rs`, and dedicated Tauri guard test files.
- Tauri thread command DTOs are owned by `src-tauri/src/services/threads/types.rs`; keep `services/threads.rs` focused on shared plan execution and native effects.
- Linux WebUI-only behavior must remain behind `SystemCapabilities` or capability gates; macOS Tauri must not expose Web auth, Turnstile, systemd, Nginx, admin password, public endpoint, or Linux prune surfaces.
- `webui/src/App.tsx` should stay an app shell for navigation, runtime/session gating, and composition. Domain workspace components belong under `webui/src/components/<domain>/` and should consume query/action hooks, domain view-model helpers, and capability props instead of importing transport, raw API functions, or React Query cache primitives.
- macOS Tauri and Linux WebUI share the same `webui` visual vocabulary. Navigation labels, core panel titles, action copy, disabled states, and cleanup dry-run gating belong in shared visual/domain contracts; host differences must come only from `SystemCapabilities`, host policy, or runtime transport.

## Verification

Use these commands for normal handoff:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/test-install-script.sh
```

`bash scripts/package-linux.sh` is canonical only on Linux x86_64. `ALLOW_HOST_MISMATCH=1` is for local smoke archives only.
