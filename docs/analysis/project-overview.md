# Project Overview

## Preliminary Direction

Build `NexusHub` as a new repo based on `codex-cloud-panel`, keep Codex local-state compatibility intact, replace the cloud Sentinel runtime with a built-in Probe surface, and add a read-only Claude Code provider framework inspired by multi-provider IDE consoles without copying AGPL source.

## Current Architecture

```mermaid
flowchart TD
    LinuxBrowser[Linux WebUI React/Vite] --> API[nexushubd Axum API]
    MacTauri[macOS Tauri App] --> API
    API --> Core[nexushub-core]
    Core --> CodexState[Codex state DB, session index, rollout files]
    Core --> Jobs[Controlled codex exec jobs]
    Core --> PanelDb[NexusHub SQLite DB]
    Core --> ClaudeHome[~/.claude read-only]
    Core --> Platform[PlatformPaths]
    API --> FixedJobs[Fixed update/doctor/archive jobs]
    Nginx[Nginx /nexushub/] --> API
```

The daemon listens on `127.0.0.1:15742` and is intended to be exposed only through an HTTPS reverse proxy under `/nexushub/`. Thread list/detail/status/Probe reads use official Codex local state. Create/send uses controlled `codex exec --json` jobs; actions that cannot be operated reliably from local state return an explicit unavailable response.

## Technology Stack

| Layer | Current | Target |
|:--|:--|:--|
| Language | Rust 2021, TypeScript | Same |
| Backend | Axum, Tokio, rusqlite | Provider-oriented Axum API |
| Frontend | React 18, Vite, TanStack Query, lucide-react | Same visual shell with provider pages |
| Build Tool | Cargo, pnpm 11.0.8, Vite | Same |
| Database | NexusHub SQLite plus official Codex DB reads | Same; no Codex schema mutation |
| Deployment | Linux systemd under `/opt/nexushub`, Nginx `/nexushub/`; macOS Tauri App | Linux WebUI plus macOS native app; Windows Service remains preview |

## Entry Points

- Backend CLI and daemon: `crates/nexushubd/src/main.rs`
- Backend API routes: `crates/nexushubd/src/api.rs`
- Core library exports: `crates/nexushub-core/src/lib.rs`
- Provider registry: `crates/nexushub-core/src/providers.rs`
- Claude Code preview: `crates/nexushub-core/src/claude_code.rs`
- Built-in Probe runtime: `crates/nexushub-core/src/probe.rs`
- Platform path model: `crates/nexushub-core/src/platform.rs`
- WebUI shell: `webui/src/App.tsx`
- WebUI API client/tests: `webui/src/lib/api.ts`, `webui/src/lib/api.test.ts`
- Linux install/update: `deploy/nexushub/install.sh`, `deploy/nexushub/update.sh`

## Build & Run

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/test-install-script.sh
```

Canonical Linux packaging is `bash scripts/package-linux.sh` on Linux x86_64. macOS packaging targets the native Tauri App entry, not a browser WebUI or LaunchAgent Web service.

## Testing Baseline

Rust has unit and integration tests in `nexushub-core`, `nexushubd`, and script validation through `scripts/test-install-script.sh`. WebUI has Vitest tests for API helpers and message-store behavior plus a TypeScript/Vite build. The Linux release and cloud deploy path have been verified for `v0.1.43` under `/nexushub/`. Current gaps are end-to-end Linux browser interaction tests, native macOS Tauri App verification, and Windows Service packaging.

## Project Governance Baseline

`AGENTS.md` is the active shared instruction surface. `CLAUDE.md` is intentionally absent because the user deleted it, and agents must not restore it unless the user explicitly requests that file. `docs/progress/MASTER.md` tracks the released Linux state. There is no repo-local memory file; durable memory remains the active agent's native memory unless the user explicitly asks for a repo fallback.

## External Integrations

- Official Codex state DB under `/root/.codex/state_5.sqlite`.
- Codex rollout/session files under Codex home.
- Controlled `codex exec --json` jobs for create/send.
- Fixed cloud Codex admin wrappers under `/home/ubuntu/codex-admin/bin`.
- Turnstile login verification and encrypted secret storage.
- GitHub Releases for `lich13/nexushub` updates.
- Nginx reverse proxy under `/nexushub/`.
- Claude Code read-only files under `~/.claude`.
