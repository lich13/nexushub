# NexusHub 1.0.0

NexusHub is a compact Codex, Grok Build and Pi task browser. One React interface serves macOS ARM64 Tauri, Linux x86_64 Tauri and a headless Linux WebUI. Deployment targets are supplied explicitly.

## Capabilities

- Codex: browse, search, filter, rename through the native CLI, archive/restore, and copy messages, IDs, paths or resume commands. Archived tasks must be restored before renaming. CLI 0.144.2 or newer is required. Native rename is verified with `thread/read`; an already-open Codex Desktop window may need reopening.
- Grok Build: read native `summary.json` / `updates.jsonl`, group tool updates, rename through fixed native ACP, copy the native thread ID, and preview/confirm deletion of one local session directory.
- Pi: read native JSONL history and the current branch, search, inspect tools, copy the native thread ID, rename through fixed native RPC, and preview/confirm deletion of one session file. No Pi installation is needed for reading existing files. Rename requires the native CLI. Active or uncertain sessions are protected from mutation.
- Probe: Codex main-task notifications, confirmed Grok completion/failure and Pi completion via Bark; persistent cursors and dedupe, with no historical replay. Pi final failure is unavailable without an extension. Goal recovery remains Codex-only and never starts a turn.
- Settings: system/update status, scoped cleanup and execution history. Account/security settings exist only on the server. Codex log database maintenance is retired; native logs are only read for error detection.

History reads never start tasks. There is no composer, send/steer/stop/fork, approval/question/plan action, manual Goal control, uploads, historical follow-up execution or desktop LAN Web service. Management never removes workspaces, worktrees or provider credentials. Existing compatibility database rows remain inert.

Batch selection accepts at most 100 loaded threads. Codex supports archive, restore and deletion of selected archived records; Grok/Pi support deletion. Preview lists per-item scope and blockers; execution rechecks fingerprints. Failures remain selected and require a new preview.

## Development

Install Rust, Node and Corepack. Use pnpm 11.0.8 and the checked-in lockfiles.

```bash
corepack pnpm@11.0.8 --dir webui install --frozen-lockfile
corepack pnpm@11.0.8 --dir webui dev --host 127.0.0.1
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui typecheck
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
corepack pnpm@11.0.8 --dir webui build:tauri
corepack pnpm@11.0.8 --dir webui exec playwright install chromium webkit
corepack pnpm@11.0.8 --dir webui test:browser
bash scripts/test-install-script.sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The independent Tauri workspace has its own lockfile. Shared actions follow the contract registry and [architecture workflow](docs/ARCHITECTURE.md). Tests run against isolated data; never delete user sessions to validate a change.

## Runtime and provider discovery

| Runtime | Persistent paths |
| --- | --- |
| macOS desktop | `~/Library/Application Support/NexusHub/` and `~/Library/Logs/NexusHub/` |
| Linux desktop | `~/.config/NexusHub/`, `~/.local/share/NexusHub/`, `~/.local/state/NexusHub/logs/` |
| Linux server | `/etc/nexushub-webd/config.toml`, `/var/lib/nexushub-webd/`, `/var/log/nexushub-webd/` |

The desktop App synchronizes its bundled `nexushub-webd` helper to the data-directory `bin/`. Its fixed Hook/monitor helper survives App closure without a listener. The Linux server runs `/usr/local/bin/nexushub-webd` under `nexushub-webd.service` on `127.0.0.1:15742`, with static files under `/usr/share/nexushub-webd/webui/`.

Codex uses its official state DB, session index, rollout files and `logs_2.sqlite`; `codex.home` is optional. Grok uses `GROK_HOME` or `~/.grok`. Pi uses `PI_CODING_AGENT_SESSION_DIR`, then `PI_CODING_AGENT_DIR/sessions`, then `~/.pi/agent/sessions`. Overrides must name trusted local directories. Pi activity detection is conservative: a relevant or unidentifiable Pi process disables mutation, including idle open sessions. Missing directories produce an empty state, not installation or configuration writes.

## Release and installation

Canonical packaging commands:

```bash
bash scripts/package-darwin-arm64.sh
bash scripts/package-linux-tauri-x86_64.sh
bash scripts/package-webd-linux-x86_64.sh
```

Run each on its native platform. Desktop scripts inject the helper, build the relative-base WebUI once via Tauri, then restore the tracked helper placeholder. `SKIP_WEBUI_INSTALL=1` reuses a frozen install; `SKIP_WEBUI_BUILD=1` validates a previously built relative-base UI. `ALLOW_HOST_MISMATCH=1` is only for local smoke archives.

The 15 Release assets comprise the macOS DMG, app-only `nexushub-darwin-arm64.tar.gz` updater, Linux `NexusHub-<version>-Linux-x86_64.AppImage` / deb / rpm, server `nexushub-webd-linux-x86_64.tar.gz`, checksums, updater signatures and `latest.json`. The headless webd tarball is not a Tauri updater asset. `latest.json` contains only `darwin-aarch64` and `linux-x86_64`. Linux Tauri requires WebKit/GTK and CI `xvfb` smoke; Tencent Cloud remains headless.

On macOS install the official DMG into `/Applications/NexusHub.app`, open the App, and verify App, bundled helper and App Support helper versions agree. Desktop updates require confirmation and signature verification. Server installation/update/recovery is documented in the [cloud runbook](docs/cloud-deploy-runbook.md).

## Documentation

- [Agent instructions](AGENTS.md): required constraints and checks.
- [Architecture](docs/ARCHITECTURE.md): module ownership and feature synchronization.
- [Design](DESIGN.md): shared layout and interaction rules.
- [Cloud runbook](docs/cloud-deploy-runbook.md): deployment and recovery.
- [Current status](docs/progress/MASTER.md): active release and acceptance evidence.

Keep exactly these six current Markdown documents. Version 1.0.0 starts a new Git history; future changes use normal incremental commits. No old-SHA restoration instructions apply.
