# NexusHub

NexusHub is a compact Codex and Grok Build task browser with a shared React interface for Tauri desktop and the Tencent Cloud Linux WebUI at `https://661313.xyz/nexushub/`.

## Scope

- Codex: browse, search, filter, rename, archive/restore, and copy messages, IDs, paths or resume commands. Copying a command never executes it.
- Grok Build: read native `summary.json` and `updates.jsonl`; merge tool updates by call identity; rename through fixed `grok agent stdio` and ACP extension `_x.ai/session/rename`.
- Grok local-file deletion requires a preview and explicit confirmation. Identity, activity, path boundaries, symlinks and content fingerprints are rechecked. Workspaces, worktrees, settings and cloud tasks are never deleted.
- No task creation, sending, follow-up queue execution, steer, stop, fork, uploads, approvals, question answers, plan acceptance or manual Goal controls.
- Probe: main-task completion/question notifications, Hook status, an event timeline and independent Bark/Goal-recovery settings.
- Settings: system/update status, maintenance and execution history; account/security settings appear only on the server WebUI.
- Desktop runs the embedded interface and fixed Hook/monitor helper only. It does not serve a desktop LAN WebUI.

The task read model uses Codex local state, session index and rollout files. History reads are side-effect free. Existing jobs, followups and shadow Goal records remain in SQLite for compatibility but cannot restart retired task execution.

The two Linux release lines have distinct roles: `nexushub-webd-linux-x86_64.tar.gz` is the headless server package; AppImage/deb/rpm are Tauri desktop packages. `latest.json` contains signed desktop updater assets for `darwin-aarch64` and `linux-x86_64`, never the server tarball. Windows and Linux arm64 are not current release targets.

The Linux desktop bundle requires WebKit/GTK build dependencies and is smoke-tested under `xvfb`; those desktop-only requirements do not belong on the headless Tencent Cloud host. The headless webd tarball is not a Tauri updater asset and is not put into `latest.json`.

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
~/Library/Logs/NexusHub/
```

Linux desktop Tauri App layout:

```text
~/.config/NexusHub/config.toml
~/.local/share/NexusHub/nexushub.sqlite
~/.local/share/NexusHub/bin/nexushub-webd
~/.local/state/NexusHub/logs/
```

On desktop, open the installed Tauri App. It synchronizes the bundled helper into the App Support data directory for Hooks and the monitor. Upgrade cleanup stops only a positively identified legacy desktop Web service and removes its dedicated static copy; shared databases, credentials and user configuration remain untouched.

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

On macOS, the native App maintains `com.lich13.nexushub.probe-error-monitor`, a background `LaunchAgent` that runs the App Support helper with the fixed `probe monitor-errors` command so terminal Codex errors can still be observed while the App is closed. This process is not a Web service: its plist has no `Sockets` entry, opens no listener, and never starts a desktop Web service. On Tencent Cloud the existing `nexushub-webd` systemd process runs the same monitor loop without installing another daemon.

## Local Build

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui install --frozen-lockfile
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

Turnstile is configured after login in `设置 / 账户与安全`. The cloud defaults match cc-switch-lite semantics: 365-day sessions, Site Key `0x4AAAAAADPfCPB_O-N3j6ON`, action `login`, expected hostname `661313.xyz`, token replay protection, and enabled login verification. The `required` switch is a fail-closed guard when Turnstile is not enabled. Secret values are encrypted at rest, write-only, and never returned by the API.

## macOS ARM64 Acceptance

After installing the DMG, validate the Tauri App directly:

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushub-webd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
```

The App is the only desktop interface. Verify the official monitor remains running after the App closes, with no listener. The App plist, bundled helper and App Support helper must report the same release version.

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

Interactive acceptance checks task browsing, Markdown/code/link rendering, menus, long text, desktop/mobile layouts, visible errors, Grok native rename and isolated test-directory deletion. No sending or manual Goal controls may appear. Both cleanup workflows stop at final confirmation without deleting user data. On the server, verify security persistence and authenticated RPC behavior; all retired and sensitive paths must return `404`.

After healthz, doctor, and public `/nexushub/` checks pass, old release-update backups can be deleted or pruned. Do not create an extra backup just to compact `logs_2.sqlite`; use the gated compact workflow and remove existing backups only after successful health verification.

## Safety Boundaries

- The panel reads Codex local state directly and does not expose Codex control endpoints.
- No arbitrary root shell is available from the WebUI.
- Maintenance actions are fixed jobs only.
- Secret fields return only configured status.
- Archive deletion requires dry-run visibility plus button confirmation; no typed confirmation text is required.
- Windows service packaging is currently a planned/preview surface, not a verified release asset.
