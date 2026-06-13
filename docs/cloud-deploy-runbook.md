# Cloud Deploy Runbook

Target host: `43.155.235.227`

## Build Release Artifact

```bash
bash scripts/package-linux.sh
sha256sum dist/nexushub-linux-x86_64.tar.gz
```

## Deploy

```bash
scp dist/nexushub-linux-x86_64.tar.gz 43.155.235.227:/tmp/
ssh 43.155.235.227 'sudo -n deploy/nexushub/install.sh --archive /tmp/nexushub-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/'
```

The second command assumes a checked-out deployment script already exists at `deploy/nexushub/install.sh` on the host. If only the release archive is present, unpack it first and run the installer from the archive's `nexushub/deploy/install.sh`.

The installed service listens on `127.0.0.1:15732`.

Ensure `/opt/nexushub/config.toml` keeps the bridge local:

```toml
[codex]
app_server_socket = "/root/.codex/app-server-control/app-server-control.sock"
bridge_enabled = true
bridge_transport = "websocket"
bridge_timeout_seconds = 20
```

Nginx should proxy only `/nexushub/` to `127.0.0.1:15732`; do not expose root app-server, `/v1`, `/responses`, or metrics publicly.

Initialize or rotate login password with a 12+ char secret:

```bash
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<strong-password>' /opt/nexushub/bin/nexushubd admin init --username admin"
ssh 43.155.235.227 "sudo NEXUSHUB_ADMIN_PASSWORD='<new-strong-password>' /opt/nexushub/bin/nexushubd admin reset-password --username admin"
```

## Verification

```bash
ssh 43.155.235.227 'sudo -n systemctl is-active nexushub'
ssh 43.155.235.227 'curl -fsS http://127.0.0.1:15732/healthz'
curl -fsS https://661313.xyz/nexushub/
ssh 43.155.235.227 'sudo -n /opt/nexushub/bin/nexushubd doctor'
```

Then log in and verify:

- thread list loads;
- system status shows `codex-app-server-root.service` active;
- create/send uses app-server bridge and returns `bridge=true`;
- `codex exec` appears only as fallback job when bridge is unavailable;
- system status shows `43.155.235.227` / `https://661313.xyz/nexushub/` instead of any removed SSH alias;
- renamed thread titles refresh from app-server `thread/read`;
- Goal settings and permission/model/config selectors load;
- Turnstile Site Key / Secret Key can be saved, action is `login`, expected hostname is `661313.xyz`, session TTL is 365 days, and token replay protection is active;
- archive delete dry-run returns counts and `integrity=ok`;
- archive delete execute uses button confirmation only, with no typed confirmation text;
- panel update and Codex update are separate cards;
- failed update jobs show structured explanation and suggested next actions;
- Codex update / prune buttons start only the configured fixed wrappers.

## Rollback

`nexushub-update` stores backups under:

```text
/opt/nexushub/backups/release-updates/<timestamp>
```

Restore binary and WebUI from the newest backup, then restart:

```bash
sudo systemctl restart nexushub
curl -fsS http://127.0.0.1:15732/healthz
```
