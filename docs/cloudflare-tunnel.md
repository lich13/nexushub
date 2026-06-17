# Cloudflare Tunnel Optional Entry

Cloudflare Tunnel is an optional public entry for an already-running NexusHub
daemon. It does not replace NexusHub authentication, the local-only listener, or
the verified Tencent Cloud Linux deployment at `https://661313.xyz/nexushub/`.

NexusHub still listens on `127.0.0.1:15742`. The tunnel origin must point to that
loopback service. Do not expose Codex control sockets, `/v1`, `/responses`, or
metrics through any Cloudflare hostname.

## Supported Modes

### Production Hostname

Use a production tunnel only when you own the Cloudflare account, zone, and
hostname. Cloudflare supports both dashboard-managed tunnel tokens and
locally-managed named tunnels. In either case, map the public hostname to:

```text
http://127.0.0.1:15742
```

For a locally-managed named tunnel, the shape is:

```yaml
tunnel: <tunnel-uuid-or-name>
credentials-file: /path/outside/repo/<tunnel-uuid>.json
ingress:
  - hostname: nexushub.example.com
    service: http://127.0.0.1:15742
  - service: http_status:404
```

Run it with the explicit config path:

```bash
cloudflared tunnel --config /path/outside/repo/config.yml run <tunnel-uuid-or-name>
```

For a dashboard-managed tunnel token, install the connector using the command
shown by Cloudflare Zero Trust. Keep the token in the local service manager or
secret store; do not copy it into this repository, release notes, release assets,
logs, screenshots, or the NexusHub WebUI.
Do not commit tunnel tokens, URL tokens, credentials JSON, Cloudflare API tokens,
or generated Quick Tunnel URLs.

Recommended Cloudflare-side checks:

- The hostname belongs to a Cloudflare zone you control.
- The tunnel route points to `http://127.0.0.1:15742`.
- If the hostname is path-prefixed for `/nexushub/`, verify the browser path and
  API calls after login. A dedicated hostname is simpler because NexusHub itself
  serves from `/`.
- Cloudflare Access is enabled for the hostname when the panel is reachable from
  the Internet. Access is recommended defense-in-depth, not a NexusHub runtime
  dependency.
- Cloudflare Access is recommended for Internet-facing hostnames, but it is not
  a NexusHub hard dependency.

## Temporary Quick Tunnel

Quick Tunnel is only for temporary preview or debugging:

```bash
cloudflared tunnel --url http://127.0.0.1:15742
```

Cloudflare documents Quick Tunnel as testing/development only. It generates a
random `trycloudflare.com` hostname, has no uptime guarantee, and is not a
production NexusHub service endpoint. Do not publish Quick Tunnel URLs as release
or acceptance endpoints.

## macOS ARM64 Local Acceptance

After installing the macOS ARM64 DMG, verify the local service before adding a
tunnel:

```bash
curl -fsS http://127.0.0.1:15742/healthz
open http://127.0.0.1:15742/nexushub/
launchctl print gui/$(id -u)/com.nexushub.nexushub
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushubd.log"
```

Expected local paths:

```text
~/Library/Application Support/NexusHub/config.toml
~/Library/Application Support/NexusHub/nexushub.sqlite
~/Library/Application Support/NexusHub/webui/
~/Library/Logs/NexusHub/
~/Library/LaunchAgents/com.nexushub.nexushub.plist
```

If a Cloudflare Tunnel is enabled on macOS, keep its LaunchDaemon/LaunchAgent
separate from `com.nexushub.nexushub`. `cloudflared` should be independently stoppable,
and NexusHub must continue to pass `healthz` on loopback after `cloudflared` is
stopped.

## Optional Helper

`deploy/nexushub/cloudflare-tunnel/cloudflared-nexushub` is a local helper for
operators who already have `cloudflared` credentials. It only checks
`cloudflared`, writes/removes a local plist when asked, and reports status. It
does not create Cloudflare accounts, zones, tunnels, DNS records, Access apps, or
tokens.

Examples:

```bash
deploy/nexushub/cloudflare-tunnel/cloudflared-nexushub check
deploy/nexushub/cloudflare-tunnel/cloudflared-nexushub install \
  --config "$HOME/.cloudflared/nexushub.yml" \
  --tunnel nexushub
deploy/nexushub/cloudflare-tunnel/cloudflared-nexushub status
deploy/nexushub/cloudflare-tunnel/cloudflared-nexushub uninstall
```

The helper intentionally has no `--token` option. Token-based dashboard-managed
tunnels should be installed from the Cloudflare dashboard command and reviewed so
the token never lands in repository files or command logs.

## Dual-Endpoint Acceptance

Linux Tencent Cloud acceptance remains:

```bash
ssh 43.155.235.227 'sudo -n systemctl is-active nexushub'
ssh 43.155.235.227 'curl -fsS http://127.0.0.1:15742/healthz'
curl -fsS https://661313.xyz/nexushub/
ssh 43.155.235.227 'sudo -n /opt/nexushub/bin/nexushubd doctor'
```

macOS ARM64 acceptance is local-first:

```bash
curl -fsS http://127.0.0.1:15742/healthz
open http://127.0.0.1:15742/nexushub/
launchctl print gui/$(id -u)/com.nexushub.nexushub
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushubd.log"
```

Cloudflare Tunnel acceptance is additive:

```bash
cloudflared tunnel info <tunnel-uuid-or-name>
curl -fsS https://<hostname>/
curl -fsS https://<hostname>/healthz
```

If Cloudflare Access is enabled, browser acceptance should first hit the Access
login challenge and then the NexusHub login page. API smoke checks with `curl`
may return an Access redirect or unauthorized response until authenticated; that
is expected.

## References

- Cloudflare Tunnel locally-managed tunnel: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/create-local-tunnel/
- Cloudflare Tunnel run parameters and macOS service notes: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/configure-tunnels/run-parameters/
- Cloudflare Quick Tunnels: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/
- Cloudflare Access self-hosted application: https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/self-hosted-public-app/
