# NexusHub

`nexushub` is a Rust + React operations console for Codex local state with one shared `webui` frontend packaged for three host surfaces: Linux server WebUI, desktop embedded Tauri, and desktop LAN WebUI. On Tencent Cloud Linux it runs as a local-only daemon exposed through Nginx HTTPS at `https://661313.xyz/nexushub/`. On macOS and Linux desktop the supported entry is the Tauri App. The Tauri App can optionally start a default-off, independently authenticated LAN WebUI service from the bundled `nexushub-webd` helper.

The Tauri apps follow the CC Switch native packaging model: Tauri wraps the main `webui` interface directly. macOS produces `NexusHub.app`, `NexusHub-<version>-darwin-arm64.dmg`, `nexushub-darwin-arm64.tar.gz`, signed updater metadata `nexushub-darwin-arm64.tar.gz.sig`, and `latest.json` platform `darwin-aarch64`. Linux desktop produces `NexusHub-<version>-Linux-x86_64.AppImage`, `.deb`, `.rpm`, and AppImage updater signature for `latest.json` platform `linux-x86_64`. The Linux server release chain builds the same `webui` into `/usr/share/nexushub-webd/webui/`, publishes `nexushub-webd-linux-x86_64.tar.gz`, and serves the hosted browser entry at `https://661313.xyz/nexushub/`.

The two Linux release lines are intentional. `nexushub-webd-linux-x86_64.tar.gz` is the headless Tencent Cloud WebUI/systemd package; `NexusHub-<version>-Linux-x86_64.AppImage`, `.deb`, and `.rpm` are the Linux Tauri desktop packages. The Linux Tauri release job is expected to take longer because it installs WebKit/GTK dependencies, builds the Tauri bundle, produces AppImage/deb/rpm, signs the AppImage when release secrets are present, and runs an `xvfb` smoke test. The headless webd tarball is not a Tauri updater asset and must not appear in `latest.json`.

Architecture parity with `cc-switch` is intentionally scoped. `cc-switch origin/main` is the desktop Tauri release reference and includes Windows plus Linux arm64; local `cc-switch feat/webd` branches are separate headless/FHS references. NexusHub keeps the accepted macOS + Tencent Cloud Linux target in this release, with Windows desktop and Linux arm64 recorded as P2 until they have explicit CI, Release, updater, and acceptance coverage.

Current scope:

- Login, HttpOnly session cookie, CSRF-protected mutating API, Turnstile settings.
- Encrypted Turnstile secret storage compatible with legacy codex-cloud-panel and cc-switch-lite key import.
- Desktop-style conversation workspace backed by Codex local state and controlled `codex exec --json` jobs.
- Thread read model from the resolved Codex home, Codex `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`.
- Running / reply-needed / recoverable / archived status cards.
- Archive delete dry-run and button-confirmed execute path with integrity checks.
- Shared update status and update jobs: Linux server uses `/usr/local/bin/nexushub-webd-update` through fixed systemd-health-checked jobs; macOS and Linux Tauri use the signed Tauri updater feed at `https://github.com/lich13/nexushub/releases/latest/download/latest.json`.
- Job failure analysis for common release, checksum, systemd, Nginx, sudo, Codex auth, SQLite, network, and local-state failures.
- Plan Mode, model, reasoning, and a compact Codex APP-style permission menu for the conversation workspace.
- Network access defaults to enabled for generated sandbox policies; the WebUI does not expose a network checkbox.
- Provider preview framework for Codex, Claude Code, future Cursor CLI, and future Gemini CLI. Codex is the only full-control provider in this release.
- Claude Code preview is read-only: it discovers `~/.claude/projects`, session JSONL files, and redacted settings. It does not launch, resume, send, stop, or write Claude configuration.
- Built-in Probe replaces the old `codex-sentinel-server` runtime path for cloud use: status, thread classification, Hook events, Bark testing, logs-db maintenance, and settings are handled inside NexusHub. It does not add hidden desktop control, automatic replies, Sentinel alias routes, or direct destructive deletion endpoints.
- Desktop navigation can be hidden to give the conversation workspace more horizontal room.
- System status, job history, Linux server WebUI, macOS/Linux Tauri App shells, and Tauri-controlled desktop LAN WebUI service status.

Thread listing, thread details, status cards, Probe, archive deletion, Plan Mode state, and logs-db maintenance read or persist NexusHub state locally from the resolved Codex home plus the NexusHub panel DB: `state_5.sqlite`, `session_index.jsonl`, rollout files, `logs_2.sqlite`, and `nexushub.sqlite`. Conversation create/send actions use controlled `codex exec --json` jobs. Stop, fork, and approvals that cannot be operated reliably from local state return an explicit unavailable response instead of depending on a root app-server socket. Historical goal/plan/choice/approval items are only surfaced when they are still the latest unresolved action.

## Runtime Layout

Linux production layout:

```text
/usr/local/bin/nexushub-webd
/etc/systemd/system/nexushub-webd.service
/usr/share/nexushub-webd/webui/
/etc/nexushub-webd/config.toml
/etc/nexushub-webd/env
/var/lib/nexushub-webd/nexushub.sqlite
/var/log/nexushub-webd/
```

The daemon listens on `127.0.0.1:15742`. Nginx should proxy public HTTPS traffic to that loopback port.
`/etc/nexushub-webd/env` must contain `NEXUSHUB_SECRET_KEY`. During migration the installer copies the old `/opt/nexushub` config, env, and SQLite files once, then normalizes runtime paths to the new layout. It preserves an existing NexusHub key first; otherwise it imports `/etc/codex-cloud-panel/env` `CODEX_CLOUD_PANEL_SECRET_KEY`, then `/etc/cc-switch-lite/env` `CC_SWITCH_LITE_SECRET_KEY`, and only generates a new key when no legacy key exists. This keeps existing encrypted Turnstile settings readable during migration.

macOS ARM64 Tauri App layout:

```text
~/Library/Application Support/NexusHub/config.toml
~/Library/Application Support/NexusHub/nexushub.sqlite
~/Library/Application Support/NexusHub/bin/nexushub-webd
~/Library/Application Support/NexusHub/desktop-assets/
~/Library/Logs/NexusHub/
```

Linux desktop Tauri App layout:

```text
~/.config/NexusHub/config.toml
~/.local/share/NexusHub/nexushub.sqlite
~/.local/share/NexusHub/bin/nexushub-webd
~/.local/share/NexusHub/desktop-assets/
~/.local/state/NexusHub/logs/
```

On desktop, open NexusHub from the installed Tauri App bundle. The App bundle carries the local `nexushub-webd` helper and syncs it into the desktop data directory on launch so Probe Bark tests, Hook installation, and optional LAN WebUI use the same controlled helper path. Do not document or ship a LaunchAgent Web service or Cloudflare Tunnel entry for desktop platforms.

Desktop LAN WebUI is controlled only from embedded Tauri:

```toml
[desktop_webui]
enabled = false
listen = "0.0.0.0:15743"
username = "admin"
session_ttl_seconds = 86400
cookie_secure = false
public_base_url = null
turnstile_enabled = false
```

The LAN WebUI password is never stored as plaintext in `config.toml`; Tauri writes an Argon2 hash in the independent `desktop-webui:<username>` admin realm. The helper starts only when `enabled=true`, a password is configured, and the listen port is available. Browser clients of the LAN WebUI get login and CSRF protection plus shared Codex/Claude Code/Probe/Ops pages, but cannot remotely start or stop the service and do not see Linux server surfaces such as systemd, Nginx, public endpoint, or Linux prune.

## Codex State

`[codex]` config controls local Codex state discovery:

```toml
workspace = "/home/ubuntu/codex-workspace"
host_label = "43.155.235.227"
```

`codex.home` is optional. When omitted, NexusHub auto-discovers the Codex home from the local state layout, normally `/root/.codex` or `/home/ubuntu/.codex`. NexusHub depends on Codex `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`; it does not require `codex-app-server-root.service`, `app_server_socket`, or bridge settings in default config. The systemd unit grants write access only to those two Codex homes plus `/etc/nexushub-webd`, `/var/lib/nexushub-webd`, and `/var/log/nexushub-webd`; any other discovered Codex home should be treated as a warning and granted explicitly rather than broadening `ReadWritePaths`.

The public site exposes `/nexushub/` for the Linux WebUI and `/nexushub/api/` for NexusHub API requests through Nginx. The host-level `/api/` namespace is reserved for other services and must not be claimed by NexusHub. Do not publish any Codex control sockets, `/v1`, `/responses`, or metrics endpoints. Legacy `/codex-cloud-panel/` and `/api/sentinel/status` paths should remain unavailable from the public panel surface.

## Probe

`[probe]` config controls the built-in Probe runtime. Probe settings are split between `config.toml` for non-sensitive values and encrypted `PanelDb.settings` entries for sensitive values such as the Bark `device_key`.

Probe routes are canonical RPC commands under the daemon-local `/api/rpc/probe.*` namespace, for example `/api/rpc/probe.status`, `/api/rpc/probe.settings.get`, and `/api/rpc/probe.logsDb.status`. Through the Linux `/nexushub/api/` proxy these remain RPC routes such as `/nexushub/api/rpc/probe.status`; old REST Probe paths return `404`, including `/api/probe/*` and `/nexushub/api/probe/*`. `/api/sentinel/*` compatibility aliases are not part of the packaged runtime. Codex `logs_2.sqlite` maintenance runs automatically in the background; compaction uses the existing DB in place after health gates instead of creating a new backup. The WebUI only displays status and metrics while settings and Bark tests use fixed, auditable actions.

The old `codex-sentinel-server` cleanup was a one-time migration and is no longer shipped as a NexusHub runtime helper. Release packages should not install `nexushub-probe-legacy-cleanup`; the live Hook handler remains `nexushub-webd probe hook-stop`.

## Local Build

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui install
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/package-webd-linux-x86_64.sh
bash scripts/package-darwin-arm64.sh
bash scripts/package-linux-tauri-x86_64.sh
```

`scripts/package-webd-linux-x86_64.sh` intentionally refuses to produce the Linux server release asset on non-Linux x86_64 hosts. It writes `dist/nexushub-webd-linux-x86_64.tar.gz` and `.sha256`. `scripts/package-linux.sh` is only a deprecated compatibility shim to the webd package script; new automation should call the webd script directly.
`scripts/package-darwin-arm64.sh` intentionally refuses to produce the macOS ARM64 release assets on non-Darwin ARM64 hosts. It uses `webui` as the Tauri frontend and writes `dist/nexushub-darwin-arm64.tar.gz`, `dist/nexushub-darwin-arm64.tar.gz.sig`, `dist/NexusHub-<version>-darwin-arm64.dmg`, and matching `.sha256` files in signed release builds. The release workflow publishes `latest.json` for `darwin-aarch64`.
`scripts/package-linux-tauri-x86_64.sh` intentionally refuses to produce Linux desktop Tauri assets on non-Linux x86_64 hosts. It writes `dist/NexusHub-<version>-Linux-x86_64.AppImage`, `.AppImage.sig` in signed release builds, `.deb`, `.rpm`, and matching `.sha256` files. The release workflow publishes the AppImage in `latest.json` for `linux-x86_64`.
The desktop packaging scripts build the release `nexushub-webd` helper, inject it into the Tauri resources for packaging, and restore the tracked `src-tauri/resources/nexushub-webd` placeholder before exit.
`ALLOW_HOST_MISMATCH=1` is only for local smoke archives and is not a canonical release path.

## Server Install

Tencent Cloud Linux remains the canonical hosted deployment:

```bash
sudo deploy/nexushub-webd/install.sh \
  --archive ./dist/nexushub-webd-linux-x86_64.tar.gz \
  --domain 661313.xyz \
  --path-prefix /nexushub/

sudo NEXUSHUB_ADMIN_PASSWORD='<strong-password>' \
  /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml admin init --username admin
```

Password must be at least 12 chars. To rotate it later:

```bash
sudo NEXUSHUB_ADMIN_PASSWORD='<new-strong-password>' \
  /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml admin reset-password --username admin
```

Turnstile is configured after login in `安全 / Security`. The cloud defaults match cc-switch-lite semantics: 365-day sessions, Site Key `0x4AAAAAADPfCPB_O-N3j6ON`, action `login`, expected hostname `661313.xyz`, token replay protection, and enabled login verification. The `required` switch is a fail-closed guard when Turnstile is not enabled. Secret values are encrypted at rest, write-only, and never returned by the API.

## macOS ARM64 Acceptance

After installing the DMG, validate the Tauri App directly:

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushub-webd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
```

The app should open as a native macOS desktop experience. A LaunchAgent Web service and Cloudflare Tunnel are not supported macOS entry points. If desktop LAN WebUI is enabled from the Tauri Ops settings, validate `http://127.0.0.1:15743/healthz`, independent password login, CSRF-protected actions, and then turn the service back off for default acceptance.

## Linux Desktop Acceptance

The Linux desktop AppImage is built and smoke-tested in GitHub Actions with `xvfb`:

```bash
test -x dist/NexusHub-<version>-Linux-x86_64.AppImage
test -s dist/NexusHub-<version>-Linux-x86_64.AppImage.sig
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.AppImage.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.deb.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.rpm.sha256
```

Tencent Cloud remains a headless Linux server WebUI deployment; it does not need GUI AppImage acceptance.

## Update

```bash
sudo /usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest
```

Use the shared update entry for updates and cleanup:

- Linux `NexusHub 更新` runs `/usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest`; its prune action removes old `/var/lib/nexushub-webd/backups/release-updates` backups while keeping the latest three.
- macOS and Linux Tauri `NexusHub 更新` check the signed Tauri updater feed at `https://github.com/lich13/nexushub/releases/latest/download/latest.json` and install only after user confirmation and signature verification.
- Archive cleanup is split into archived-thread cleanup and hidden-thread cleanup, each with a dry-run and confirmation step.

The configured commands run fixed wrappers only, redact sensitive output, and attach a structured explanation when a job fails.

## Deploy Verification

Tencent Cloud Linux:

```bash
sudo systemctl is-active nexushub-webd
curl -fsS http://127.0.0.1:15742/healthz
curl -fsS https://661313.xyz/nexushub/
sudo /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml doctor
shasum -a 256 -c dist/nexushub-webd-linux-x86_64.tar.gz.sha256
```

macOS ARM64:

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushub-webd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
shasum -a 256 -c dist/nexushub-darwin-arm64.tar.gz.sha256
test -s dist/nexushub-darwin-arm64.tar.gz.sig
test -s dist/latest.json
shasum -a 256 -c dist/NexusHub-<version>-darwin-arm64.dmg.sha256
```

Linux Tauri desktop:

```bash
test -x dist/NexusHub-<version>-Linux-x86_64.AppImage
test -s dist/NexusHub-<version>-Linux-x86_64.AppImage.sig
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.AppImage.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.deb.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.rpm.sha256
```

Current interactive acceptance requires Chrome 插件验收. Log in there and verify: thread list loads from local Codex state, system status shows the IP/public endpoint and resolved Codex state paths, conversation send works through controlled `codex exec --json` jobs, Plan Mode and the compact permission menu work, old goal/plan threads do not show stale pending prompts, Turnstile settings persist, the panel update card works, archive and hidden-thread delete dry-runs report `integrity=ok`, Probe uses `/api/rpc/probe.status`, old REST Probe paths return `404`, and both `/codex-cloud-panel/` and `/api/sentinel/status` remain `404`.

After healthz, doctor, and public `/nexushub/` checks pass, old release-update backups can be deleted or pruned. Do not create an extra backup just to compact `logs_2.sqlite`; use the gated compact workflow and remove existing backups only after successful health verification.

## Safety Boundaries

- The panel reads Codex local state directly and does not expose Codex control endpoints.
- No arbitrary root shell is available from the WebUI.
- Maintenance actions are fixed jobs only.
- Secret fields return only configured status.
- Archive deletion requires dry-run visibility plus button confirmation; no typed confirmation text is required.
- Windows service packaging is currently a planned/preview surface, not a verified release asset.
