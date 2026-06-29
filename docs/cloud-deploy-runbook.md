# Cloud Deploy Runbook

Target host: `43.155.235.227`

This runbook is for the Tencent Cloud Linux server deployment only. It keeps the
public service at `https://661313.xyz/nexushub/`, the systemd unit `nexushub-webd`,
and the FHS-style runtime under `/usr/local/bin`, `/usr/share/nexushub-webd`,
`/etc/nexushub-webd`, `/var/lib/nexushub-webd`, and `/var/log/nexushub-webd`. Do not mix these Linux server paths with
macOS or Linux Tauri desktop layouts.

The Linux server chain remains the hosted WebUI deployment chain. macOS and
Linux Tauri builds consume the same `webui` source as the main interface and can
optionally expose a Tauri-controlled desktop LAN WebUI, but they do not add a
LaunchAgent, systemd user service, Cloudflare Tunnel, or Tencent Cloud GUI
requirement.

Release assets intentionally keep separate Linux responsibilities. The cloud
deployment uses only `nexushub-webd-linux-x86_64.tar.gz` and its `.sha256`; Linux
Tauri desktop assets are `NexusHub-<version>-Linux-x86_64.AppImage`, `.deb`,
`.rpm`, signatures, and checksums. The Linux Tauri job normally takes longer
than the headless webd package because it installs WebKit/GTK dependencies,
builds Tauri bundles, packages AppImage/deb/rpm, signs the updater asset, and
runs `xvfb` smoke. Do not deploy the AppImage to Tencent Cloud and do not put
the headless webd tarball into `latest.json`.

For `cc-switch` comparison, keep the reference boundary explicit: `cc-switch
origin/main` has a wider desktop matrix, including Windows and Linux arm64,
while local `cc-switch feat/webd` branches are separate headless/FHS references.
NexusHub server deployment remains Tencent Cloud Linux x86_64 in this runbook.

## Build Release Artifact

```bash
bash scripts/package-webd-linux-x86_64.sh
sha256sum dist/nexushub-webd-linux-x86_64.tar.gz
sha256sum -c dist/nexushub-webd-linux-x86_64.tar.gz.sha256
```

The release workflow also produces:

```text
dist/nexushub-webd-linux-x86_64.tar.gz
dist/nexushub-webd-linux-x86_64.tar.gz.sha256
dist/nexushub-darwin-arm64.tar.gz
dist/nexushub-darwin-arm64.tar.gz.sig
dist/nexushub-darwin-arm64.tar.gz.sha256
dist/NexusHub-<version>-darwin-arm64.dmg
dist/NexusHub-<version>-darwin-arm64.dmg.sha256
dist/NexusHub-<version>-Linux-x86_64.AppImage
dist/NexusHub-<version>-Linux-x86_64.AppImage.sig
dist/NexusHub-<version>-Linux-x86_64.AppImage.sha256
dist/NexusHub-<version>-Linux-x86_64.deb
dist/NexusHub-<version>-Linux-x86_64.deb.sha256
dist/NexusHub-<version>-Linux-x86_64.rpm
dist/NexusHub-<version>-Linux-x86_64.rpm.sha256
dist/latest.json
```

`latest.json` is the signed Tauri updater manifest for `darwin-aarch64` and `linux-x86_64`. The macOS URL points at `nexushub-darwin-arm64.tar.gz` and must match `nexushub-darwin-arm64.tar.gz.sig`; the Linux desktop URL points at `NexusHub-<version>-Linux-x86_64.AppImage` and must match the AppImage `.sig`.

## Deploy

```bash
scp dist/nexushub-webd-linux-x86_64.tar.gz 43.155.235.227:/tmp/
ssh 43.155.235.227 'rm -rf /tmp/nexushub-webd-linux-x86_64 && tar -xzf /tmp/nexushub-webd-linux-x86_64.tar.gz -C /tmp'
ssh 43.155.235.227 'sudo -n /tmp/nexushub-webd-linux-x86_64/deploy/install.sh --archive /tmp/nexushub-webd-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/'
```

The packaged `scripts/deploy-cloud.sh` performs the same copy, unpack, install, service, and public smoke sequence.

The installed service listens on `127.0.0.1:15742`.
The installed unit file is `/etc/systemd/system/nexushub-webd.service`.

For migrations from `codex-cloud-panel` and the retired `/opt/nexushub` layout, keep the encrypted Turnstile secret readable by preserving the old encryption key. The installer copies an existing `/opt/nexushub/env` into `/etc/nexushub-webd/env` once when needed and preserves `NEXUSHUB_SECRET_KEY`; otherwise it imports `/etc/codex-cloud-panel/env` `CODEX_CLOUD_PANEL_SECRET_KEY`, then `/etc/cc-switch-lite/env` `CC_SWITCH_LITE_SECRET_KEY`, and only generates a new key if no legacy key exists. Verify by comparing hashes only, never by printing secret values.

Ensure `/etc/nexushub-webd/config.toml` keeps Codex state local:

```toml
[codex]
workspace = "/home/ubuntu/codex-workspace"
host_label = "43.155.235.227"
```

Omit `codex.home` unless a custom home is required. NexusHub auto-discovers the Codex home and reads `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`; systemd write access is intentionally limited to `/root/.codex`, `/home/ubuntu/.codex`, `/etc/nexushub-webd`, `/var/lib/nexushub-webd`, and `/var/log/nexushub-webd`. If a different Codex home is discovered, treat it as a warning and add that path deliberately.

Nginx should proxy `/nexushub/` to `127.0.0.1:15742` and `/nexushub/api/` to the daemon-local `/api/` routes. Do not proxy the host-level `/api/` namespace to NexusHub because Sub2API and other host services own it. NexusHub must not expose Codex control sockets, arbitrary shell, `/nexushub/v1`, `/nexushub/responses`, or `/nexushub/metrics`; host-root `/v1`, `/responses`, and `/metrics` can belong to non-NexusHub gateway services and should be audited as separate ownership. Retired `/codex-cloud-panel/` and Sentinel compatibility paths such as `/api/sentinel/status` should stay unavailable from the public panel surface.

Initialize or rotate login password with a 12+ char secret:

```bash
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<strong-password>' /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml admin init --username admin"
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<new-strong-password>' /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml admin reset-password --username admin"
```

## Verification

Linux systemd and public HTTPS checks:

```bash
ssh 43.155.235.227 'sudo -n systemctl is-active nexushub-webd'
ssh 43.155.235.227 'curl -fsS http://127.0.0.1:15742/healthz'
curl -fsS https://661313.xyz/nexushub/
curl -fsS https://661313.xyz/nexushub/api/public/settings
curl -sS -o /dev/null -w '%{http_code}\n' https://661313.xyz/codex-cloud-panel/
curl -sS -o /dev/null -w '%{http_code}\n' https://661313.xyz/api/sentinel/status
curl -sS -i https://661313.xyz/api/v1/models | head -n 20
ssh 43.155.235.227 'sudo -n /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml doctor'
sha256sum -c dist/nexushub-webd-linux-x86_64.tar.gz.sha256
```

Then log in through Chrome 插件验收 and verify:

- thread list loads;
- system status shows resolved Codex state paths without requiring `codex-app-server-root.service`;
- create/send starts controlled `codex exec --json` jobs and returns a job-backed response;
- system status shows `43.155.235.227` / `https://661313.xyz/nexushub/` instead of any removed SSH alias;
- thread titles refresh from local state DB, `session_index.jsonl`, and rollout metadata without plan-body pollution;
- Plan Mode and permission/model/config selectors load;
- Turnstile Site Key / Secret Key can be saved, action is `login`, expected hostname is `661313.xyz`, session TTL is 365 days, and token replay protection is active;
- archive delete dry-run returns counts and `integrity=ok`;
- archive delete execute uses button confirmation only, with no typed confirmation text;
- panel update remains the only WebUI-exposed update card;
- failed update jobs show structured explanation and suggested next actions;
- panel prune removes old NexusHub release-update backups while keeping the latest three;
- retired local maintenance routes stay unavailable from the WebUI and HTTP API.

Expected retired path results: `/codex-cloud-panel/` and `/api/sentinel/status` return `404`. The root `/api/v1/...` namespace must not return NexusHub's `{"error":"not found"}` response; it should continue to be handled by Sub2API.
NexusHub-scoped sensitive paths such as `/nexushub/v1`, `/nexushub/responses`, and `/nexushub/metrics` must return `404` or another non-NexusHub unavailable response. If host-root `/v1`, `/responses`, or `/metrics` returns content, confirm the owning gateway service before changing it.

## Desktop Tauri Boundary

macOS and Linux Tauri acceptance are app-local and separate from this cloud runbook. macOS uses the official DMG; Linux desktop uses CI `xvfb` smoke for the AppImage because Tencent Cloud stays headless.

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushub-webd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
shasum -a 256 -c dist/nexushub-darwin-arm64.tar.gz.sha256
test -s dist/nexushub-darwin-arm64.tar.gz.sig
test -s dist/latest.json
shasum -a 256 -c dist/NexusHub-<version>-darwin-arm64.dmg.sha256
test -x dist/NexusHub-<version>-Linux-x86_64.AppImage
test -s dist/NexusHub-<version>-Linux-x86_64.AppImage.sig
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.AppImage.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.deb.sha256
shasum -a 256 -c dist/NexusHub-<version>-Linux-x86_64.rpm.sha256
```

Expected macOS paths:

```text
~/Library/Application Support/NexusHub/
~/Library/Application Support/NexusHub/bin/nexushub-webd
~/Library/Application Support/NexusHub/desktop-assets/
~/Library/Logs/NexusHub/
```

Expected Linux desktop paths:

```text
~/.config/NexusHub/config.toml
~/.local/share/NexusHub/bin/nexushub-webd
~/.local/share/NexusHub/desktop-assets/
~/.local/state/NexusHub/logs/
```

The Tauri App bundle carries the local `nexushub-webd` helper and syncs it into the
desktop data directory on launch. This helper is used for Probe Bark tests, Hook
installation, and the optional desktop LAN WebUI. The LAN WebUI is default-off,
requires an independent `desktop-webui:<username>` password, disables Turnstile
by default, and must be started or stopped only from embedded Tauri. Browser
clients of the LAN WebUI must not see Linux server systemd, Nginx, public
endpoint, or prune controls.

## Cleanup

Use the built-in gated compact path for Codex `logs_2.sqlite`; do not create a new manual backup for compaction. After `systemctl`, `healthz`, public `/nexushub/`, and `doctor` all pass, remove existing obsolete backups or run the panel prune action.

## Rollback

`nexushub-webd-update` stores backups under:

```text
/var/lib/nexushub-webd/backups/release-updates/<timestamp>
```

Restore binary and WebUI from the newest backup, then restart:

```bash
sudo systemctl restart nexushub-webd
curl -fsS http://127.0.0.1:15742/healthz
```
