# Linux management API — 1.2.1

Supply the SSH host, HTTPS domain and archive path explicitly. Private values stay outside Git. The server has no website and does not install Pi or a Linux desktop app.

## Runtime and isolation

- Binary/unit: `/usr/local/bin/nexushub-webd`, `/etc/systemd/system/nexushub-webd.service`.
- Config/env: `/etc/nexushub-webd/config.toml`, `/etc/nexushub-webd/env`.
- Database/logs: `/var/lib/nexushub-webd/`, `/var/log/nexushub-webd/`.
- Listener: loopback only; public Nginx exposes the chosen prefix's `api/rpc/` and `healthz`. Other paths return 404.

Keep `ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true` and `PrivateTmp=true`. Grok/Pi session roots use exact optional `ReadWritePaths`; missing provider directories do not prevent startup. Creating a directory or changing a custom root requires regenerating the unit/drop-in, `daemon-reload` and restart. A host rw mount can remain ro inside systemd; chmod cannot override it.

Grok's native leader socket and lock use `/var/lib/nexushub-webd/grok-leader.sock` through `GROK_LEADER_SOCKET`. This keeps native rename available with a read-only provider home; credentials and configuration remain protected. Native mutation working directories must be visible in the service namespace, so host `/tmp` fixtures are unsuitable with `PrivateTmp=true`.

## Installation and administrator Key

The seven release assets remain macOS DMG/checksum, updater archive/signature, `latest.json` for `darwin-aarch64`, and Linux webd tarball/checksum. The Linux archive contains the API binary and deployment tools only. Verify the exact approved tag and hashes.

```bash
NEXUSHUB_DOMAIN=api.example.com bash scripts/deploy-cloud.sh SSH_HOST /absolute/staging/nexushub-webd-linux-x86_64.tar.gz
sudo /usr/local/bin/nexushub-webd admin key-generate --output /absolute/private/api-key
```

The Key output file must not already exist and is created with mode 0600. Enter the value in the macOS App's 远程连接 settings, using the HTTPS base prefix. Delete the temporary Key file after it is saved in Keychain. The server stores only the digest.

```bash
sudo /usr/local/bin/nexushub-webd admin key-rotate --output /absolute/private/new-api-key
sudo /usr/local/bin/nexushub-webd admin key-revoke
```

Rotation immediately invalidates the old Key; update the App connection. Revocation or a missing Key denies all business requests. Never pass a Key in a URL, shell argument, log or repository file. Browser cookies cannot authenticate.

## Migration and recovery

The upgrade removes NexusHub web administrators/sessions, Turnstile data and retired settings, then erases SQLite freed pages and WAL. Bark encryption material, events, delivery dedupe, jobs and business audit survive. Config migration removes old website settings. Provider sessions and native Codex databases stay untouched.

Only retained runtime files/data may have one task-level recovery copy. Do not archive the retired website or login database. Remove the static directory and obsolete web updater; preserve shared Nginx, TLS and unrelated paths. Future rollback targets must be API-era releases. A failed update restores retained service state, then checks health before retrying.

## Acceptance

```bash
sudo /usr/local/bin/nexushub-webd --version
sudo systemctl is-active nexushub-webd
sudo systemctl show nexushub-webd -p MainPID -p ProtectHome -p ProtectSystem -p ReadWritePaths
curl -fsS http://127.0.0.1:15742/healthz
```

Verify public health succeeds, old pages/assets/login return 404 and unauthenticated RPC returns 401. In the official App, connect to the API, read disposable sessions/attachments, perform an allowed management action and switch back to local data. Verify wrong, rotated and revoked keys, target isolation, failed connections, pending writes, server-path copying and local Plan saving.

Keep the native asynchronous-question tracker, final-reply notification classification, provider terminal evidence and dedupe as regressions. App closure must not stop either machine's monitor. A cloud host without Pi is covered by empty state and isolated readers, not a claim of native Pi mutation acceptance. Remove only task-created staging, test data and recovery files after acceptance.
