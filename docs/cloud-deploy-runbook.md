# Headless Linux deployment

Supply the SSH host, public domain and archive path explicitly. Keep private values and acceptance evidence outside Git. This runbook covers the Linux `webd` service; it does not install Pi or a Linux desktop app.

## Runtime and isolation

- Binary: `/usr/local/bin/nexushub-webd`
- Unit: `/etc/systemd/system/nexushub-webd.service`
- UI: `/usr/share/nexushub-webd/webui/`
- Config/env: `/etc/nexushub-webd/config.toml`, `/etc/nexushub-webd/env`
- Database/logs: `/var/lib/nexushub-webd/`, `/var/log/nexushub-webd/`
- Listener: loopback only; place the public reverse proxy path in the explicit deployment command.

Keep `ProtectSystem=full`, `ProtectHome=read-only`, `NoNewPrivileges=true` and `PrivateTmp=true`. Default Grok and Pi session roots use an optional, exact `ReadWritePaths` allowlist. A missing default directory is prefixed with `-` so service startup does not fail. Creating a directory or changing a custom root requires the packaged unit/drop-in, `daemon-reload` and a restart.

Inspect service policy without changing data:

```bash
sudo systemctl show nexushub-webd -p MainPID -p ProtectHome -p ProtectSystem -p ReadWritePaths
sudo systemctl status nexushub-webd --no-pager
```

A host filesystem mounted rw can still be ro inside the service namespace. Check both mount views and fix the exact allowlist; chmod cannot override a namespace mount policy.

## Release and install

`v1.1.9` publishes seven assets: macOS DMG/checksum, macOS updater archive/signature, `latest.json` with only `darwin-aarch64`, and the Linux webd tarball/checksum. Build the webd tarball on Linux x86_64 and verify its checksum. It is not a Tauri updater asset.

```bash
sudo deploy/nexushub-webd/install.sh --archive /absolute/staging/nexushub-webd-linux-x86_64.tar.gz --domain panel.example --path-prefix /nexushub/
NEXUSHUB_DOMAIN=panel.example bash scripts/deploy-cloud.sh SSH_HOST /absolute/staging/nexushub-webd-linux-x86_64.tar.gz
```

Use an exact approved tag for acceptance. Existing database, authentication, Turnstile and encrypted Bark settings remain in place. Do not put a real hostname, IP, credential or session in the repository.

## Migration and acceptance

Opening the database drops NexusHub's retired `codex_thread_goals` table, rebuilds `probe_error_incidents` without recovery columns and preserves incident rows. Codex logs remain read-only. Old Goal commands are unavailable; no recovery attempt or notification is scheduled.

```bash
sudo /usr/local/bin/nexushub-webd --version
sudo systemctl is-active nexushub-webd
curl -fsS http://127.0.0.1:15742/healthz
```

The 1.1.9 upgrade expands only NexusHub-owned PreToolUse hooks to native synchronous/asynchronous questions and preserves unrelated hooks. Verify that the monitor establishes the async baseline before a dedicated new question. An accepted acknowledgement, further commands and normal completion must leave it pending; native matched answers, cancellation and replacement stop it. Verify the device receives the push while the desktop App is closed, and record latency separately from HTTP delivery. Use an isolated cloud session for the same chain and label that evidence as a fixture. Keep native question bubbles, AGENTS.md folding and historical timelines as regressions. This release adds no RPC, database table or native-session migration.

Use the authenticated public entry with disposable sessions to verify Grok batch deletion, Pi empty state, running spinner, mixed Grok tool folding, plan copy/download, provider Bark delivery and copy-ID feedback. A host without Pi is not evidence of Pi mutation support.

On failure restore the previous service binary/unit and verify health before retrying. Remove only task-created staging and test paths after acceptance; retain user data, configuration and secrets.

## User attachment acceptance

After a 1.1.9 update, verify user bubbles and attachments through the authenticated HTTPS entrypoint. Test an embedded native image and an expired file reference: the former previews while the latter shows an unavailable card. A read request uses only session/message/attachment identities; unauthenticated reads and caller-supplied paths must fail. Preserve the existing systemd sandbox. Server previews only use server-side session data and files; a client-only temporary path can remain unavailable.
