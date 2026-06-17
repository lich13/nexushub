# NexusHub

`nexushub` is a Rust + React web panel for cloud Codex local state. It runs as a local-only daemon on the server and is exposed through Nginx HTTPS.

Current scope:

- Login, HttpOnly session cookie, CSRF-protected mutating API, Turnstile settings.
- Encrypted Turnstile secret storage compatible with legacy codex-cloud-panel and cc-switch-lite key import.
- Desktop-style conversation workspace backed by Codex local state and controlled `codex exec --json` jobs.
- Thread read model from the resolved Codex home, Codex `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`.
- Running / reply-needed / recoverable / archived status cards.
- Archive delete dry-run and button-confirmed execute path with integrity checks.
- Panel update jobs through `/usr/local/bin/nexushub-update`; retired local maintenance actions are not exposed from the WebUI or HTTP API.
- Job failure analysis for common release, checksum, systemd, Nginx, sudo, Codex auth, SQLite, network, and local-state failures.
- Plan Mode, model, reasoning, and a compact Codex APP-style permission menu for the conversation workspace.
- Network access defaults to enabled for generated sandbox policies; the WebUI does not expose a network checkbox.
- Provider preview framework for Codex, Claude Code, future Cursor CLI, and future Gemini CLI. Codex is the only full-control provider in this release.
- Claude Code preview is read-only: it discovers `~/.claude/projects`, session JSONL files, and redacted settings. It does not launch, resume, send, stop, or write Claude configuration.
- Built-in Probe replaces the old `codex-sentinel-server` runtime path for cloud use: status, thread classification, Hook events, Bark testing, logs-db maintenance, and settings are handled inside NexusHub. It does not add hidden desktop control, automatic replies, Sentinel alias routes, or direct destructive deletion endpoints.
- Desktop navigation can be hidden to give the conversation workspace more horizontal room.
- System status, job history, and responsive sky-blue dark WebUI.

Thread listing, thread details, status cards, Probe, archive deletion, Plan Mode state, and logs-db maintenance read or persist NexusHub state locally from the resolved Codex home plus the NexusHub panel DB: `state_5.sqlite`, `session_index.jsonl`, rollout files, `logs_2.sqlite`, and `nexushub.sqlite`. Conversation create/send actions use controlled `codex exec --json` jobs. Stop, fork, and approvals that cannot be operated reliably from local state return an explicit unavailable response instead of depending on a root app-server socket. Historical goal/plan/choice/approval items are only surfaced when they are still the latest unresolved action.

## Runtime Layout

```text
/opt/nexushub/bin/nexushubd
/opt/nexushub/config.toml
/opt/nexushub/env
/opt/nexushub/nexushub.sqlite
/opt/nexushub/logs/
/opt/nexushub/webui/
```

The daemon listens on `127.0.0.1:15742`. Nginx should proxy public HTTPS traffic to that loopback port.
`/opt/nexushub/env` must contain `NEXUSHUB_SECRET_KEY`. The installer preserves an existing NexusHub key first; otherwise it imports `/etc/codex-cloud-panel/env` `CODEX_CLOUD_PANEL_SECRET_KEY`, then `/etc/cc-switch-lite/env` `CC_SWITCH_LITE_SECRET_KEY`, and only generates a new key when no legacy key exists. This keeps existing encrypted Turnstile settings readable during migration.

## Codex State

`[codex]` config controls local Codex state discovery:

```toml
workspace = "/home/ubuntu/codex-workspace"
host_label = "43.155.235.227"
```

`codex.home` is optional. When omitted, NexusHub auto-discovers the Codex home from the local state layout, normally `/root/.codex` or `/home/ubuntu/.codex`. NexusHub depends on Codex `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`; it does not require `codex-app-server-root.service`, `app_server_socket`, or bridge settings in default config. The systemd unit grants write access only to those two Codex homes plus `/opt/nexushub`; any other discovered Codex home should be treated as a warning and granted explicitly rather than broadening `ReadWritePaths`.

The public site must expose only `/nexushub/` through Nginx. Do not publish any Codex control sockets, `/v1`, `/responses`, or metrics endpoints. Legacy `/codex-cloud-panel/` and `/api/sentinel/status` paths should remain unavailable from the public panel surface.

## Probe

`[probe]` config controls the built-in Probe runtime. Probe settings are split between `config.toml` for non-sensitive values and encrypted `PanelDb.settings` entries for sensitive values such as the Bark `device_key`.

Probe routes are canonical under `/api/probe/*`. `/api/sentinel/*` compatibility aliases are not part of the packaged runtime. Codex `logs_2.sqlite` maintenance runs automatically in the background; compaction uses the existing DB in place after health gates instead of creating a new backup. The WebUI only displays status and metrics while settings and Bark tests use fixed, auditable actions.

The old `codex-sentinel-server` cleanup was a one-time migration and is no longer shipped as a NexusHub runtime helper. Release packages should not install `nexushub-probe-legacy-cleanup`; the live Hook handler remains `nexushubd probe hook-stop`.

## Local Build

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui install
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/package-linux.sh
```

`scripts/package-linux.sh` intentionally refuses to produce the Linux release asset on non-Linux hosts. Use the GitHub Actions release workflow for the canonical Linux x86_64 tarball.
`ALLOW_HOST_MISMATCH=1` is only for local smoke archives and is not a canonical release path.

## Server Install

```bash
sudo deploy/nexushub/install.sh \
  --archive ./dist/nexushub-linux-x86_64.tar.gz \
  --domain 661313.xyz \
  --path-prefix /nexushub/

sudo NEXUSHUB_ADMIN_PASSWORD='<strong-password>' \
  /opt/nexushub/bin/nexushubd admin init --username admin
```

Password must be at least 12 chars. To rotate it later:

```bash
sudo NEXUSHUB_ADMIN_PASSWORD='<new-strong-password>' \
  /opt/nexushub/bin/nexushubd admin reset-password --username admin
```

Turnstile is configured after login in `安全 / Security`. The cloud defaults match cc-switch-lite semantics: 365-day sessions, Site Key `0x4AAAAAADPfCPB_O-N3j6ON`, action `login`, expected hostname `661313.xyz`, token replay protection, and enabled login verification. The `required` switch is a fail-closed guard when Turnstile is not enabled. Secret values are encrypted at rest, write-only, and never returned by the API.

## Update

```bash
sudo /usr/local/bin/nexushub-update --repo lich13/nexushub --version latest
```

Use the WebUI Ops page for panel updates and cleanup:

- `面板更新` runs `/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest`; its prune action removes old `/opt/nexushub/backups/release-updates` backups while keeping the latest three.
- Archive cleanup is split into archived-thread cleanup and hidden-thread cleanup, each with a dry-run and confirmation step.

The configured commands run fixed wrappers only, redact sensitive output, and attach a structured explanation when a job fails.

## Deploy Verification

```bash
sudo systemctl is-active nexushub
curl -fsS http://127.0.0.1:15742/healthz
curl -fsS https://661313.xyz/nexushub/
sudo /opt/nexushub/bin/nexushubd doctor
```

Current interactive acceptance requires Chrome 插件验收. Log in there and verify: thread list loads from local Codex state, system status shows the IP/public endpoint and resolved Codex state paths, conversation send works through controlled `codex exec --json` jobs, Plan Mode and the compact permission menu work, old goal/plan threads do not show stale pending prompts, Turnstile settings persist, the panel update card works, archive and hidden-thread delete dry-runs report `integrity=ok`, and both `/codex-cloud-panel/` and `/api/sentinel/status` remain `404`.

After healthz, doctor, and public `/nexushub/` checks pass, old release-update backups can be deleted or pruned. Do not create an extra backup just to compact `logs_2.sqlite`; use the gated compact workflow and remove existing backups only after successful health verification.

## Safety Boundaries

- The panel reads Codex local state directly and does not expose Codex control endpoints.
- No arbitrary root shell is available from the WebUI.
- Maintenance actions are fixed jobs only.
- Secret fields return only configured status.
- Archive deletion requires dry-run visibility plus button confirmation; no typed confirmation text is required.
- Windows service packaging is currently a planned/preview surface, not a verified release asset.
