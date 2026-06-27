# nexushub-webd Linux deployment

## Install

```bash
sudo deploy/install.sh --archive ./nexushub-webd-linux-x86_64.tar.gz --domain 661313.xyz --path-prefix /nexushub/
sudo NEXUSHUB_ADMIN_PASSWORD='change-me-long-password' \
  nexushub-webd --config /etc/nexushub-webd/config.toml admin init --username admin
sudo systemctl restart nexushub-webd
```

The install step writes:

```text
/usr/local/bin/nexushub-webd
/usr/share/nexushub-webd/webui
/etc/nexushub-webd/config.toml
/etc/nexushub-webd/env
/var/lib/nexushub-webd/nexushub.sqlite
/var/log/nexushub-webd
```

When `/opt/nexushub` exists, the installer copies the existing config, env, and
SQLite files once before normalizing paths to the new layout.

## Update

```bash
sudo /usr/local/bin/nexushub-webd-update latest
```

## Rollback

```bash
sudo deploy/rollback.sh v0.1.140
```

## Verify

```bash
systemctl is-active nexushub-webd
/usr/local/bin/nexushub-webd --version
curl -fsS http://127.0.0.1:15742/healthz
curl -fsS https://661313.xyz/nexushub/
```
