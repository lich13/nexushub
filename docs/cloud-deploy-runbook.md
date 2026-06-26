# Cloud Deploy Runbook

Target host: `43.155.235.227`

This runbook is for the Tencent Cloud Linux deployment only. It keeps the public
service at `https://661313.xyz/nexushub/`, the systemd unit `nexushub`, and the
runtime under `/opt/nexushub`. Do not mix these Linux paths with the macOS ARM64
Tauri App layout. macOS no longer provides a browser WebUI, LaunchAgent Web
service, or Cloudflare Tunnel entry.

The Linux chain remains the hosted WebUI deployment chain. The macOS Tauri build
now consumes the same `webui` source as the main interface, but packages it as an
App-only native bundle; it does not add any macOS browser service.

## Build Release Artifact

```bash
bash scripts/package-linux.sh
sha256sum dist/nexushub-linux-x86_64.tar.gz
sha256sum -c dist/nexushub-linux-x86_64.tar.gz.sha256
```

The release workflow also produces:

```text
dist/nexushub-linux-x86_64.tar.gz
dist/nexushub-linux-x86_64.tar.gz.sha256
dist/nexushub-darwin-arm64.tar.gz
dist/nexushub-darwin-arm64.tar.gz.sig
dist/nexushub-darwin-arm64.tar.gz.sha256
dist/NexusHub-<version>-darwin-arm64.dmg
dist/NexusHub-<version>-darwin-arm64.dmg.sha256
dist/latest.json
```

`latest.json` is the signed Tauri updater manifest for `darwin-aarch64`; its URL points at the release `nexushub-darwin-arm64.tar.gz` asset and its signature must match `nexushub-darwin-arm64.tar.gz.sig`.

## Deploy

```bash
scp dist/nexushub-linux-x86_64.tar.gz 43.155.235.227:/tmp/
ssh 43.155.235.227 'sudo -n deploy/nexushub/install.sh --archive /tmp/nexushub-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/'
```

The second command assumes a checked-out deployment script already exists at `deploy/nexushub/install.sh` on the host. If only the release archive is present, unpack it first and run the installer from the archive's `nexushub/deploy/install.sh`.

The installed service listens on `127.0.0.1:15742`.

For migrations from `codex-cloud-panel`, keep the encrypted Turnstile secret readable by preserving the old encryption key. The installer preserves an existing `/opt/nexushub/env` `NEXUSHUB_SECRET_KEY`; otherwise it imports `/etc/codex-cloud-panel/env` `CODEX_CLOUD_PANEL_SECRET_KEY`, then `/etc/cc-switch-lite/env` `CC_SWITCH_LITE_SECRET_KEY`, and only generates a new key if no legacy key exists. Verify by comparing hashes only, never by printing secret values.

Ensure `/opt/nexushub/config.toml` keeps Codex state local:

```toml
[codex]
workspace = "/home/ubuntu/codex-workspace"
host_label = "43.155.235.227"
```

Omit `codex.home` unless a custom home is required. NexusHub auto-discovers the Codex home and reads `state_5.sqlite`, `session_index.jsonl`, rollout files, and `logs_2.sqlite`; systemd write access is intentionally limited to `/root/.codex`, `/home/ubuntu/.codex`, and `/opt/nexushub`. If a different Codex home is discovered, treat it as a warning and add that path deliberately.

Nginx should proxy `/nexushub/` to `127.0.0.1:15742` and `/nexushub/api/` to the daemon-local `/api/` routes. Do not proxy the host-level `/api/` namespace to NexusHub because Sub2API and other host services own it. NexusHub must not expose Codex control sockets, arbitrary shell, `/nexushub/v1`, `/nexushub/responses`, or `/nexushub/metrics`; host-root `/v1`, `/responses`, and `/metrics` can belong to non-NexusHub gateway services and should be audited as separate ownership. Retired `/codex-cloud-panel/` and Sentinel compatibility paths such as `/api/sentinel/status` should stay unavailable from the public panel surface.

Initialize or rotate login password with a 12+ char secret:

```bash
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<strong-password>' /opt/nexushub/bin/nexushubd admin init --username admin"
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<new-strong-password>' /opt/nexushub/bin/nexushubd admin reset-password --username admin"
```

## Verification

Linux systemd and public HTTPS checks:

```bash
ssh 43.155.235.227 'sudo -n systemctl is-active nexushub'
ssh 43.155.235.227 'curl -fsS http://127.0.0.1:15742/healthz'
curl -fsS https://661313.xyz/nexushub/
curl -fsS https://661313.xyz/nexushub/api/public/settings
curl -sS -o /dev/null -w '%{http_code}\n' https://661313.xyz/codex-cloud-panel/
curl -sS -o /dev/null -w '%{http_code}\n' https://661313.xyz/api/sentinel/status
curl -sS -i https://661313.xyz/api/v1/models | head -n 20
ssh 43.155.235.227 'sudo -n /opt/nexushub/bin/nexushubd doctor'
sha256sum -c dist/nexushub-linux-x86_64.tar.gz.sha256
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

## macOS ARM64 Boundary

macOS acceptance is app-local and separate from this cloud runbook:

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushubd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
shasum -a 256 -c dist/nexushub-darwin-arm64.tar.gz.sha256
test -s dist/nexushub-darwin-arm64.tar.gz.sig
test -s dist/latest.json
shasum -a 256 -c dist/NexusHub-<version>-darwin-arm64.dmg.sha256
```

Expected macOS paths:

```text
~/Library/Application Support/NexusHub/
~/Library/Application Support/NexusHub/bin/nexushubd
~/Library/Logs/NexusHub/
```

The macOS App bundle carries the local `nexushubd` helper and syncs it into
`Application Support` on launch. This helper is used for Probe Bark tests and
Hook installation; macOS still does not expose a browser WebUI service.

Do not add a browser WebUI, LaunchAgent Web service, or Cloudflare Tunnel as a
macOS entry point. The Tencent Cloud Linux WebUI remains available only at
`https://661313.xyz/nexushub/`.

## Cleanup

Use the built-in gated compact path for Codex `logs_2.sqlite`; do not create a new manual backup for compaction. After `systemctl`, `healthz`, public `/nexushub/`, and `doctor` all pass, remove existing obsolete backups or run the panel prune action.

## Rollback

`nexushub-update` stores backups under:

```text
/opt/nexushub/backups/release-updates/<timestamp>
```

Restore binary and WebUI from the newest backup, then restart:

```bash
sudo systemctl restart nexushub
curl -fsS http://127.0.0.1:15742/healthz
```
