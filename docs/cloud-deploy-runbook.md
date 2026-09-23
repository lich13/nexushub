# Tencent Cloud deployment

Supply the SSH host and public domain explicitly. Keep private deployment parameters and acceptance evidence outside Git. The server stays headless; desktop installation is described in README.

## Runtime and isolation

- Binary `/usr/local/bin/nexushub-webd`; unit `/etc/systemd/system/nexushub-webd.service`; static UI `/usr/share/nexushub-webd/webui/`.
- Configuration `/etc/nexushub-webd/config.toml`; secrets `/etc/nexushub-webd/env`; SQLite `/var/lib/nexushub-webd/nexushub.sqlite`; logs `/var/log/nexushub-webd/`.
- Listener `127.0.0.1:15742`. Nginx proxies `/nexushub/` and `/nexushub/api/` only. Host-root `/api/`, `/responses` and `/metrics` may belong to other services.
- Keep `ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true` and `PrivateTmp=true`. Existing Codex/application write roots remain; optional Grok/Pi session roots are the only added writable provider paths. Never make the whole home, provider config or workspace writable.
- Native provider defaults are rooted under the service user's HOME (`/root`). Grok uses GROK_HOME/sessions; Pi uses PI_CODING_AGENT_SESSION_DIR, PI_CODING_AGENT_DIR/sessions or ~/.pi/agent/sessions. Custom roots require an explicit ReadWritePaths drop-in for the exact trusted session root. Do not follow symlinks or grant a parent directory as a workaround.
- A '-' prefix allows a missing default session directory without startup failure. If a directory is created after the service starts, restart the service so its mount namespace receives the write exception. Changing a custom root also requires an updated drop-in, daemon-reload and restart. Missing Pi does not install Pi or create its configuration.

Read-only session deletion errors can be diagnosed without touching files:

```bash
sudo systemctl show nexushub-webd -p MainPID -p ProtectHome -p ProtectSystem -p ReadWritePaths
sudo systemctl status nexushub-webd --no-pager
```

Inspect the target with `findmnt` inside the service MainPID's mount namespace and on the host. Host rw with service ro indicates a missing service exception; chmod cannot change that mount policy. Apply the packaged unit/update, then verify the exception and continued isolation of adjacent configuration/workspaces.

## Release assets

Build `scripts/package-webd-linux-x86_64.sh` on Linux x86_64. Deploy only `nexushub-webd-linux-x86_64.tar.gz` after verifying its `.sha256`. The headless webd tarball is not a Tauri updater asset.

The same Release must contain 15 assets: server tarball/checksum; macOS DMG/checksum and app-only updater tarball/signature/checksum; `NexusHub-<version>-Linux-x86_64.AppImage`/signature/checksum, deb/checksum, rpm/checksum; and `latest.json`. The updater has only darwin-aarch64 and linux-x86_64. WebKit/GTK and xvfb are Linux desktop build/smoke requirements, not cloud runtime dependencies.

## Install and update

Use a task-specific staging directory and the archive from the exact approved tag. Record its SHA-256 and the currently running version before replacement. The extracted installer accepts:

```bash
sudo deploy/nexushub-webd/install.sh --archive /absolute/staging/nexushub-webd-linux-x86_64.tar.gz --domain panel.example.com --path-prefix /nexushub/
sudo /usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version v1.0.0
```

For routine updates the fixed wrapper also accepts `--version latest`; release acceptance always uses an exact tag. Existing service config, database, secrets and login state must survive. Do not reset passwords, bypass Turnstile or clear rate limits for acceptance.

Migration from old /opt/nexushub paths copies config/env/SQLite once and normalizes known old runtime paths. Preserve an existing NEXUSHUB_SECRET_KEY first; otherwise import the legacy codex-cloud-panel or cc-switch-lite key before generating one. This preserves encrypted Turnstile/Bark settings. Compare hashes without exposing keys. Codex home discovery uses official local state; it must not depend on codex-app-server-root.service, app_server_socket or bridge settings.

New installs require an explicit `--domain`; updates retain the saved public URL, host label, authentication, Turnstile and encrypted Bark settings. A missing saved Turnstile site key must be explicitly configured before upgrade if the old binary supplied it as a default. Server account/security settings use 365-day sessions and encrypted write-only secrets. Probe settings keep non-sensitive values in config.toml and sensitive Bark values in encrypted database settings. Canonical Probe commands use `/api/rpc/probe.*`; old REST/Sentinel routes stay retired. Codex log database maintenance is retired; configuration migration removes `probe.logs_db` and preserves its event retention preference under `probe.observability.event_retention_days`. Native log error detection is read-only.

## Acceptance and recovery

```bash
sudo /usr/local/bin/nexushub-webd --version
sudo systemctl is-active nexushub-webd
curl -fsS http://127.0.0.1:15742/healthz
sudo /usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml doctor
curl -fsS https://panel.example.com/nexushub/
```

Then use the authenticated public Browser entry for real task browsing, Grok native naming, ID copying and confirmed removal of a disposable session. Test Pi's actual host state; a host without Pi is not evidence of native Pi rename. Keep its isolated adapter tests separately identified. Verify fresh logs, restart count, version/hash and authentication/CSRF. `/codex-cloud-panel/`, legacy Probe REST/Sentinel routes and NexusHub-scoped `/v1`, `/responses`, `/metrics` and control sockets remain unavailable; do not alter other services' host-root routes.

Before deployment retain at most one task-level recovery copy of changed runtime assets and the original unit, plus an exact-tag restore reference. Keep database/config/secrets in place. On failure restore the previous assets/unit and confirm service, health and public behavior before retrying. The installed update wrapper has its own managed release-updates backups; pruning retains its configured latest three. Never delete unknown backups or user data.

After all acceptance passes, remove only the task's resolved staging/download/test paths and no-longer-needed recovery copy. Record removed bytes, retained artifacts and recovery tag/hash in the current status document.

For scripted deployment use `NEXUSHUB_DOMAIN=panel.example.com bash scripts/deploy-cloud.sh SSH_HOST /absolute/path/release.tar.gz` with actual values supplied outside Git. Verify batch operations on disposable data and actual Bark delivery. A cloud host without Pi needs empty-state and isolated interface checks, not a Pi installation.
