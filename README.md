# NexusHub

`nexushub` is a Rust + React web panel for a cloud Codex app-server. It runs as a local-only daemon on the server and is exposed through Nginx HTTPS.

Current scope:

- Login, HttpOnly session cookie, CSRF-protected mutating API, Turnstile settings.
- Encrypted Turnstile secret storage compatible with legacy codex-cloud-panel and cc-switch-lite key import.
- Desktop-style conversation workspace backed by the controlled app-server bridge.
- Thread read model from `/root/.codex/state_5.sqlite`, `session_index.jsonl`, and rollout files.
- Running / reply-needed / recoverable / archived status cards.
- Archive delete dry-run and button-confirmed execute path with integrity checks.
- Split Panel update and Codex update jobs. Panel updates use `/usr/local/bin/nexushub-update`; Codex updates keep the existing `/home/ubuntu/codex-admin/bin` wrappers.
- Job failure analysis for common release, checksum, systemd, Nginx, sudo, Codex auth, SQLite, network, and app-server failures.
- Goal Mode, model, reasoning, cwd, and a compact Codex APP-style permission menu for the conversation workspace.
- Network access defaults to enabled for generated sandbox policies; the WebUI does not expose a network checkbox.
- Provider preview framework for Codex, Claude Code, future Cursor CLI, and future Gemini CLI. Codex is the only full-control provider in this release.
- Claude Code preview is read-only: it discovers `~/.claude/projects`, session JSONL files, and redacted settings. It does not launch, resume, send, stop, or write Claude configuration.
- Built-in Probe replaces the old `codex-sentinel-server` runtime path for cloud use: status, thread classification, Hook events, Bark testing, logs-db maintenance, and settings are handled inside NexusHub. It does not add hidden desktop control, automatic replies, Sentinel alias routes, or direct destructive deletion endpoints.
- Desktop navigation can be hidden to give the conversation workspace more horizontal room.
- System status, job history, and responsive sky-blue dark WebUI.

Conversation create/send/stop and thread actions use the private app-server bridge first. `codex exec --json` is kept only as fallback for create/send when the bridge is unavailable. Plan accept/revise actions use explicit panel endpoints and are marked `fallback=true` because the current app-server exposes Plan content as turn items rather than a dedicated Plan accept API; approval prompts require a live app-server JSON-RPC request connection and are shown as unsupported instead of being silently converted to text. Historical Plan/choice/approval items are only surfaced when they are still the latest unresolved action.

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

## App-Server Bridge

`[codex]` config controls the bridge:

```toml
app_server_service = "codex-app-server-root.service"
app_server_socket = "/root/.codex/app-server-control/app-server-control.sock"
bridge_enabled = true
bridge_transport = "websocket"
bridge_timeout_seconds = 20
```

The public site must expose only `nexushub` through Nginx. Do not publish the root app-server socket, `/v1`, `/responses`, or metrics endpoints. If a response has `fallback=true`, check Job History for the `codex exec` fallback job.

## Probe

`[probe]` config controls the built-in Probe runtime. Probe settings are split between `config.toml` for non-sensitive values and encrypted `PanelDb.settings` entries for sensitive values such as the Bark `device_key`.

Probe routes are canonical under `/api/probe/*`. `/api/sentinel/*` compatibility aliases are not part of the packaged runtime. Codex `logs_2.sqlite` maintenance runs automatically in the background; the WebUI only displays status and metrics while settings and Bark tests use fixed, auditable actions.

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

Use the WebUI Ops page for split updates:

- `面板更新` runs `/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest`; its prune action removes old `/opt/nexushub/backups/release-updates` backups while keeping the latest three.
- `Codex 更新` runs the existing cloud Codex wrapper chain for precheck / update / prune.

The configured commands run fixed wrappers only, redact sensitive output, and attach a structured explanation when a job fails.

## Deploy Verification

```bash
sudo systemctl is-active nexushub
curl -fsS http://127.0.0.1:15742/healthz
curl -fsS https://661313.xyz/nexushub/
sudo /opt/nexushub/bin/nexushubd doctor
```

Then log in and verify: thread list loads, system status shows the IP/public endpoint and `codex-app-server-root.service` active, bridge send returns `bridge=true`, renamed thread titles refresh from app-server `thread/read`, Goal and the compact permission menu work, old Plan Mode threads do not show stale pending prompts, Turnstile settings persist, both update cards work, and archive delete dry-run reports `integrity=ok`.

## Safety Boundaries

- The panel does not expose root Codex app-server directly; bridge access stays local.
- No arbitrary root shell is available from the WebUI.
- Maintenance actions are fixed jobs only.
- Secret fields return only configured status.
- Archive deletion requires dry-run visibility plus button confirmation; no typed confirmation text is required.
- Windows service packaging is currently a planned/preview surface, not a verified release asset.
