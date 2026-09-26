# NexusHub 1.1.2

NexusHub is a read-first workspace for Codex, Grok Build and Pi sessions. The shared React UI runs in the macOS ARM64 Tauri app and the headless Linux `webd` service. Deployment hosts, domains and credentials are supplied explicitly and stay outside Git.

## Capabilities

- **Codex**: search, read, rename through the native app-server protocol, archive/restore, batch archive/restore/delete of explicit records, and copy IDs or paths.
- **Grok Build**: read native sessions, search, rename, copy the native thread ID, batch-delete selected session directories, and inspect tool activity.
- **Pi**: read native JSONL branches, search, rename through the fixed native RPC when available, copy the native thread ID, and batch-delete selected session files. Missing or uncertain activity is read-only.
- **Probe**: read-only error detection, persistent dedupe and Bark delivery. Codex, Grok and Pi completion notifications follow provider-specific evidence rules; Pi final-failure notification remains disabled without an extension.
- **UI**: running sessions share one accessible spinner. Consecutive command activity is normalized into an outer group with one collapsible row per command; completed groups and commands start closed, while active or failed items stay open. Every visible assistant reply in Codex, Grok and Pi has a raw-Markdown copy button, and list rows support double-click rename when the provider allows it.

NexusHub has no Goal management or automatic recovery path. Codex native logs remain read-only inputs for error detection. The retired `codex_thread_goals` table is removed during database migration. Composer, send/steer/stop/fork, manual approvals/questions/plans, uploads and the desktop LAN WebUI remain unavailable.

Batch operations accept at most 100 explicit session keys. Preview results show scope, size, fingerprints and blockers. Execution rechecks identity, activity, symlinks and fingerprints; failures remain visible and require a new preview.

## Development

```bash
corepack pnpm@11.0.8 --dir webui install --frozen-lockfile
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui typecheck
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
corepack pnpm@11.0.8 --dir webui build:tauri
corepack pnpm@11.0.8 --dir webui test:browser
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
bash scripts/test-install-script.sh
```

`src-tauri` is an independent Cargo workspace with its own lockfile. Tests use disposable provider data and never delete user sessions.

## Runtime paths

| Surface | Data and logs |
| --- | --- |
| macOS app | `~/Library/Application Support/NexusHub/`, `~/Library/Logs/NexusHub/` |
| Linux server | `/etc/nexushub-webd/`, `/var/lib/nexushub-webd/`, `/var/log/nexushub-webd/` |

Codex uses its official local state DB, session index, rollouts and logs. Grok and Pi roots are discovered from their standard environment variables or user directories. Missing provider directories produce an empty state and do not install software or write configuration.

## Release and deployment

Release `v1.1.2` publishes seven files: macOS DMG and checksum, macOS updater archive and signature, `latest.json` with only `darwin-aarch64`, and the Linux webd tarball and checksum. Linux Tauri desktop packages and desktop updater entries are retired. The server tarball is never placed in `latest.json`.

```bash
bash scripts/package-darwin-arm64.sh
bash scripts/package-webd-linux-x86_64.sh
```

Use the exact approved tag and verify sidecar hashes before installation. Cloud deployment keeps `HOST`, domain and archive paths explicit:

```bash
NEXUSHUB_DOMAIN=panel.example bash scripts/deploy-cloud.sh SSH_HOST /absolute/staging/nexushub-webd-linux-x86_64.tar.gz
```

See [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md) for systemd isolation, writable session roots, upgrades and recovery.

## Current documents

- [AGENTS.md](AGENTS.md): development constraints and gates.
- [DESIGN.md](DESIGN.md): shared visual and interaction rules.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): module boundaries and contracts.
- [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md): headless deployment.
- [docs/progress/MASTER.md](docs/progress/MASTER.md): current status and evidence.

Keep exactly these six Markdown documents. Future work uses normal Git commits.
