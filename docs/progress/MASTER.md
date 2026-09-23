# Current status

- Version: `1.0.0` implementation in progress.
- Scope: three-provider batch management, Grok/Pi Bark completion monitoring, Codex log-maintenance retirement, privacy cleanup and one-time Git history rebuild.
- Current Markdown set: README.md, AGENTS.md, DESIGN.md, docs/ARCHITECTURE.md, docs/cloud-deploy-runbook.md and this file.

## Evidence

- Core and WebUI batch fixtures cover explicit keys, mixed allowed/protected items, duplicate Pi native IDs, filter races, mobile layouts and partial failure.
- Native parser tests cover Pi current branches, retry/tool failures, incomplete lines and Grok primary/cancel/failure matching.
- The official deletion acceptance for the previous release was supplied by the user. Fresh 1.0.0 batch, Bark, installed App and cloud acceptance remains required.
- Cloud service and saved security settings were rechecked read-only; deployment is unchanged.

## Remaining gates

Run all Rust/Tauri/WebUI/install/contract/privacy gates, create the parentless 1.0.0 root commit, force-update the existing `main` with an explicit lease, wait for matching CI, tag and verify the signed 15-asset release, then install and exercise the real macOS and authenticated cloud entry points. Report any old GitHub cache that remains reachable.
