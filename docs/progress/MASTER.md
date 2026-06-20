# NexusHub Merge and Framework Expansion - Progress Tracker

> **Task**: Continue NexusHub from the codex-cloud-panel base, preserve Codex behavior, replace the cloud Sentinel runtime with built-in Probe surfaces, and keep the Claude Code provider read-only.
> **Started**: 2026-06-13
> **Last Updated**: 2026-06-20
> **Mode**: V0.1.112_CC_SWITCH_UNIFIED_ARCHITECTURE_BACKFILL

## References

- [Project Overview](../analysis/project-overview.md)
- [Module Inventory](../analysis/module-inventory.md)
- [Risk Assessment](../analysis/risk-assessment.md)
- [Task Breakdown](../plan/task-breakdown.md)
- [Dependency Graph](../plan/dependency-graph.md)
- [Milestones](../plan/milestones.md)

## Phase Summary

| Phase | Name | Tasks | Done | Progress |
|:--|:--|--:|--:|:--|
| 1 | Governance Baseline | 3 | 3 | 100% |
| 2 | Codex Non-Regression | 3 | 3 | 100% |
| 3 | Provider Framework | 3 | 3 | 100% |
| 4 | WebUI Information Architecture | 3 | 3 | 100% |
| 5 | Three-Platform Service Model | 3 | 3 | 100% |
| 6 | Verification and Release Readiness | 2 | 2 | 100% |

## Phase Checklist

- [x] Phase 1: Governance Baseline (3/3 tasks) - [details](./phase-1-governance-baseline.md)
- [x] Phase 2: Codex Non-Regression (3/3 tasks) - [details](./phase-2-codex-non-regression.md)
- [x] Phase 3: Provider Framework (3/3 tasks) - [details](./phase-3-provider-framework.md)
- [x] Phase 4: WebUI Information Architecture (3/3 tasks) - [details](./phase-4-webui-information-architecture.md)
- [x] Phase 5: Three-Platform Service Model (3/3 tasks) - [details](./phase-5-three-platform-service-model.md)
- [x] Phase 6: Verification and Release Readiness (2/2 tasks) - [details](./phase-6-verification-and-release-readiness.md)

## Current Status

**Active Phase**: v0.1.112 cc-switch unified architecture backfill<br>
**Active Task**: `v0.1.112` closes the remaining unified-architecture gaps: thread steer/follow-up semantics are shared in core, desktop upload validation uses the same batch plan as HTTP upload, frontend actions move behind query/state facades, explicit cleanup capabilities gate cross-platform maintenance, macOS keeps Web security/admin/systemd/Nginx surfaces unavailable, and `CLAUDE.md` remains intentionally absent.
**Blockers**: None. Current Linux rendered WebUI acceptance requires Chrome 插件验收 for logged-in QA; macOS acceptance is native Tauri App validation through Computer Use.

## Governance Status

**Shared instruction surface**: `AGENTS.md`<br>
**Deleted local instruction surface**: `CLAUDE.md` is intentionally absent and must not be restored unless the user explicitly requests it.<br>
**Other platform rule surfaces**: none detected<br>
**Memory surface**: native memory only<br>
**Memory fallback path**: none

## Adaptive Control State

```yaml
adaptive:
  mode: V0.1.112_CC_SWITCH_UNIFIED_ARCHITECTURE_BACKFILL
  strategy: "conservative provider shell around preserved Codex behavior"
  phases:
    phase_1:
      drift_score: 0
      thresholds: { annotate: 1, replan: 2, rescope: 2 }
      total_tasks: 3
      completed_tasks: 3
    phase_2:
      drift_score: 0
      thresholds: { annotate: 1, replan: 2, rescope: 2 }
      total_tasks: 3
      completed_tasks: 3
    phase_3:
      drift_score: 0
      thresholds: { annotate: 1, replan: 2, rescope: 2 }
      total_tasks: 3
      completed_tasks: 3
    phase_4:
      drift_score: 0
      thresholds: { annotate: 1, replan: 2, rescope: 2 }
      total_tasks: 3
      completed_tasks: 3
    phase_5:
      drift_score: 0
      thresholds: { annotate: 1, replan: 2, rescope: 2 }
      total_tasks: 3
      completed_tasks: 3
    phase_6:
      drift_score: 0
      thresholds: { annotate: 1, replan: 1, rescope: 2 }
      total_tasks: 2
      completed_tasks: 2
  last_updated: "2026-06-19"
```

## Task Telemetry Log

| Date | Task | Actual Effort | S.U.P.E.R Score | Unplanned Dependencies | Notes |
|:--|:--|:--|:--|--:|:--|
| 2026-06-13 | 1.1 | S | P/R pass | 0 | Added agent instruction surfaces |
| 2026-06-13 | 1.2 | M | P/E/R pass | 0 | Added spec docs and later updated them to released Linux state |
| 2026-06-13 | 1.3 | S | E/R pass | 0 | README DB path and preview scope aligned |
| 2026-06-13 | 2.2 | S | E/R pass | 0 | Cookie/upload/log/test fixture NexusHub rename with targeted tests |
| 2026-06-13 | 3.1-3.3 | M | S/P/R pass | 0 | Provider, Claude, and initial Sentinel-compatible endpoints added in prior pass |
| 2026-06-14 | Probe replacement | XL | S/P/E/R pass locally | 0 | Built-in Probe config/API/events/UI/deploy cleanup path replaces `codex-sentinel-server` runtime while preserving safety boundaries |
| 2026-06-14 | Probe replacement QA | M | P/R pass | 0 | Full local verification passed for `v0.1.46`; rendered Probe page verified on desktop and mobile, including Hook plan -> confirm flow |
| 2026-06-14 | Probe legacy import patch | S | P/R pass | 0 | Added real `probe legacy-import` for old `/etc/codex-sentinel-server/config.toml`, storing Bark device key only in encrypted settings; full local verification passed for `v0.1.47` |
| 2026-06-14 | Probe cloud replacement | M | P/R pass | 0 | Released and deployed `v0.1.47`; imported old config, installed NexusHub root Hook, verified logs-db dry-run/Bark/event ingest, then backed up and removed old Sentinel runtime from `43.155.235.227` |
| 2026-06-15 | Probe panel slim/settings refresh | M | S/P/R pass | 0 | Released `v0.1.48` with the Mac App style Probe panel, runtime config refresh after settings save, canonical settings payloads, and local rendered QA |
| 2026-06-15 | Probe default migration follow-up | S | P/R pass | 0 | Released and deployed `v0.1.49`; updater now migrates known old Probe defaults to Mac App defaults, asset sha256 `70a56e0d1d85caea32a248d60e64b2cb3a91bb00a17d682c95aa038e4ece235d`, and cloud config now shows `500/1000/5242880` plus logs-db `2d/6h` defaults |
| 2026-06-15 | Probe one-time helper package cleanup | S | P/R pass | 0 | Install/update packaging and docs no longer ship the removed legacy Sentinel cleanup helper; Probe defaults and `nexushubd probe hook-stop` stay canonical |
| 2026-06-15 | Probe logs-db retarget and panel slim | L | P/R pass | 0 | `v0.1.52` release candidate retargets maintenance to Codex `logs_2.sqlite`, adds scheduled automatic cleanup with gated compaction, removes stale manual Probe/Sentinel routes, and keeps Probe UI read-only for logs-db maintenance |
| 2026-06-15 | Codex path auto-discovery | M | P/R pass | 0 | `v0.1.53` resolves Codex home/socket/logs paths from config, env, socket, root/ubuntu homes, and `/home/*/.codex`; UI supports auto home without writing `/root/.codex` back. |
| 2026-06-15 | Task C deploy/docs auto-discovery | S | P/R pass | 0 | Deploy config omits fixed `codex.home`, systemd grants `/root/.codex` and `/home/ubuntu/.codex`, and docs cover no-new-backup compact plus post-health backup cleanup. |
| 2026-06-17 | v0.1.96 dual-entry docs | S | P/R pass | 0 | Documented macOS ARM64 DMG local acceptance, preserved Tencent Cloud Linux `/opt/nexushub` systemd acceptance, and added optional Cloudflare Tunnel guidance with no token storage in repo/logs/assets/WebUI. |
| 2026-06-17 | v0.1.97 macOS base path patch | S | P/R pending | 0 | Bumped workspace/package versions to `0.1.97`, fixed local `/nexushub/` static WebUI routing, changed macOS DMG packaging to derive the default version from Cargo metadata, and changed release asset upload paths to version globs. |
| 2026-06-17 | v0.1.98 Mac Tauri platform split | S | P/R pending | 0 | Bumped workspace/package versions to `0.1.98`, kept Linux WebUI public entry at `https://661313.xyz/nexushub/`, documented macOS as Tauri App only, and removed Cloudflare Tunnel from the project capability docs. |
| 2026-06-17 | v0.1.99 CC Switch style native Tauri alignment | S | P/R pass | 0 | Bumped workspace/package/Tauri versions to `0.1.99`, changed macOS Tauri and CI/release packaging to use the shared `webui` interface, removed the stale `desktop-ui` surface, wired macOS desktop API actions for Probe settings/jobs, uploads, Goal, cleanup, Job History, and thread operations through Tauri invoke, and verified Rust/WebUI/Tauri checks plus local App/DMG build. |
| 2026-06-18 | v0.1.100 macOS parity Probe helper and UI alignment | M | P/R pending | 0 | Bumped workspace/package/Tauri versions to `0.1.100`, routed desktop runtime shared actions through the native API bridge, fixed macOS Probe settings/events/logs-db behavior, added bundled `nexushubd` helper sync for Bark/Hook actions, hid Linux-only Ops update actions on macOS, and tightened Probe/composer/right-inspector UI alignment. |
| 2026-06-18 | v0.1.102 unified runtime signed updates | L | P/R pending | 0 | Bumped workspace/package/Tauri versions to `0.1.102`, added shared update status/service contracts, routed Linux HTTP and macOS Tauri update status through shared services, added signed updater packaging for `nexushub-darwin-arm64.tar.gz.sig` and `latest.json`, and kept Windows out of scope. |
| 2026-06-18 | v0.1.102 cc-switch architecture alignment | L | P/R pass locally | 0 | Added shared `threads`, `jobs`, `settings`, and `system` services; thinned Linux HTTP and macOS Tauri typed commands onto shared DTO/action contracts; removed frontend production dependence on the desktop API bridge and old panel update routes; added capability-matrix gates and static tests for Linux-only WebUI surfaces. |
| 2026-06-18 | v0.1.103 macOS updater status closure | S | P/R pending | 0 | Bumped workspace/package/Tauri versions to `0.1.103`, made macOS `desktop_update_status` remember the latest signed updater check job, and kept the UI `NexusHub 更新` card from reverting to `Latest unknown` after a successful `Check`. |
| 2026-06-18 | v0.1.104 cc-switch architecture closure | L | P/R pending | 0 | Bumped workspace/package/Tauri versions to `0.1.104`; closes remaining shared service, typed command, runtime transport, capability matrix, and static guard gaps so Linux WebUI remains a Linux-only extra host while macOS stays native Tauri. |
| 2026-06-19 | v0.1.107 cc-switch runtime RPC closure | L | P/R pass | 0 | Released `0.1.107`; moved frontend Web traffic to `/api/rpc/:command`, kept runtime as a thin transport, centralized Probe settings save normalization in core, removed macOS `desktop_security_status`, preserved `CLAUDE.md` as intentionally deleted, and fixed macOS updater stale-cache downgrade availability. |
| 2026-06-20 | v0.1.108 cc-switch typed RPC closure | M | P/R pass locally | 0 | Bumps workspace/package/Tauri versions to `0.1.108`; removes the frontend domain route table, routes shared actions through runtime RPC/Tauri typed commands, adds Linux RPC DTO alias compatibility, keeps macOS Web security commands unregistered, and adds static guards against old desktop API bridges. |
| 2026-06-20 | v0.1.109 cc-switch service closure | M | P/R pass locally | 0 | Bumps workspace/package/Tauri versions to `0.1.109`; moves Goal DTO/status/view planning and Linux update shell job specs into core services, removes the last update prune runtime-kind branch from the frontend domain API, and preserves `CLAUDE.md` as intentionally absent. |
| 2026-06-20 | v0.1.111 cc-switch final service-layer closure | M | P/R pass locally | 0 | Bumps workspace/package/Tauri versions to `0.1.111`; centralizes upload planning in core, splits frontend API implementations into domain modules behind a thin barrel, keeps runtime as transport only, removes retired desktop `_command` wrappers from registration, and keeps Linux-only WebUI surfaces capability-gated. |
| 2026-06-20 | v0.1.112 cc-switch unified architecture backfill | M | P/R pass locally | 0 | Bumps workspace/package/Tauri versions to `0.1.112`; shares steer/follow-up planning and desktop upload batch validation through core, adds explicit cleanup capabilities, moves App actions behind query/state facades, keeps typed Tauri commands free of retired string action multiplexers, and preserves `CLAUDE.md` as intentionally absent. |
| 2026-06-13 | 4.1-4.3 | M | S/P/R pass | 0 | WebUI preview navigation added in prior pass |
| 2026-06-13 | 5.1-5.3 | M | E/R pass | 0 | Platform paths and Linux migration verified in prior pass |
| 2026-06-13 | 2.1, 2.3 | M | U/P/R pass | 0 | Full Rust workspace tests passed; bridge/state read model preserved |
| 2026-06-13 | 6.1 | M | P/R pass | 0 | fmt, Rust tests, clippy, WebUI tests/build, and install script checks passed |
| 2026-06-13 | 6.2 | S | E/R pass | 0 | GitHub release and cloud deployment verified for `v0.1.43` |

## Quick Status Commands

```bash
git status --short --branch
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/test-install-script.sh
git ls-remote --tags origin refs/tags/v0.1.112
```

## Next Steps

1. Keep `origin` pointed at `https://github.com/lich13/nexushub`.
2. For every future release, wait for CI and Release workflows, verify release assets, deploy to `43.155.235.227`, and smoke `https://661313.xyz/nexushub/`.
3. For macOS ARM64, verify the Tauri App with `open -a NexusHub` and `~/Library/Logs/NexusHub`; do not add a browser WebUI, LaunchAgent Web service, or Cloudflare Tunnel entry.
4. Keep Cloudflare Turnstile login verification intact; do not confuse it with the removed Cloudflare Tunnel ingress docs.
5. Keep the retired legacy `/codex-cloud-panel/` path returning `404`; NexusHub is the public Linux WebUI surface under `/nexushub/`.

## v0.1.112 Acceptance Matrix

| Platform | Entry | Service | Runtime paths | Required checks |
|:--|:--|:--|:--|:--|
| Tencent Cloud Linux | `https://661313.xyz/nexushub/` | systemd `nexushub` | `/opt/nexushub` with packaged `webui` | `systemctl is-active`, loopback `healthz`, public HTTPS smoke, `nexushubd doctor`, retired paths `404`, Linux tarball `.sha256`, shared `NexusHub 更新` status, update job history |
| macOS ARM64 | Tauri App bundle wrapping shared `webui` | native app process | `~/Library/Application Support/NexusHub`, `~/Library/Application Support/NexusHub/bin/nexushubd`, `~/Library/Logs/NexusHub` | `open -a NexusHub`, no Web login/API admin setup, Codex/Probe/Goal/cleanup smoke, helper sync check, log tail, DMG/tarball `.sha256`, signed updater `.sig`, `latest.json`, Tauri updater `Check` shows the latest signed version in the shared update card and Job History |

## Session Log

| Date | Session | Summary |
|:--|:--|:--|
| 2026-06-13 | fresh-context continuation | Continued previous NexusHub migration, added governance docs, fixed stale non-migration names, prepared for full verification |
| 2026-06-13 | verification | Verified local handoff with Rust/WebUI/script checks; current logged-in rendered acceptance uses Chrome 插件验收 |
| 2026-06-13 | release-deploy | Published `v0.1.43`, deployed `/nexushub/` to `43.155.235.227`, migrated admin/settings from the legacy panel DB, and verified both new and old services stay active on separate loopback ports |
| 2026-06-14 | probe-replacement | Reworked Probe from Sentinel preview toward built-in runtime: settings, events/dedupe, hook/logs-db/Bark actions, Chinese Probe UI, install/update config injection, and legacy cleanup helper |
| 2026-06-14 | probe-local-qa | Bumped to `v0.1.46`; full local verification passed; Probe rendered QA passed locally |
| 2026-06-14 | probe-cloud-replacement | Published `v0.1.47`, verified release asset sha256 `9f1675818a4a5a77e1392724f309c07cedbbea8aaf6692005cd74dc615f57bbd`, deployed to `43.155.235.227`, imported legacy config, installed `/root/.codex/hooks.json` NexusHub Probe hook, confirmed Bark/logs-db/event health gates, backed up old Sentinel runtime to `/opt/nexushub/backups/probe-legacy/20260614-181532`, removed old service/runtime paths, and verified both `/nexushub/` and `/codex-cloud-panel/` return HTTP 200 |
| 2026-06-14 | codex-cloud-panel-retirement | Confirmed NexusHub covers the legacy `codex-cloud-panel` surface, removed the remaining cloud runtime files without backing them up per user instruction, added explicit Nginx `404` rules for `/codex-cloud-panel` and `/codex-cloud-panel/`, and reverified `/nexushub/` remains HTTP 200 |
| 2026-06-15 | probe-panel-slim-settings-refresh | Published `v0.1.48`, verified CI/Release, deployed to `43.155.235.227`, then found the live config still had old Probe defaults because the updater only inserted missing keys |
| 2026-06-15 | probe-default-migration-follow-up | Published `v0.1.49`, verified release asset sha256 `70a56e0d1d85caea32a248d60e64b2cb3a91bb00a17d682c95aa038e4ece235d`, deployed with a second updater pass so the newly installed updater migrated `/opt/nexushub/config.toml`, confirmed `nexushubd 0.1.49`, health OK, `/nexushub/` HTTP 200, and `/codex-cloud-panel/` HTTP 404 |
| 2026-06-15 | probe-one-time-helper-package-cleanup | Removed the packaged `nexushub-probe-legacy-cleanup` deploy helper from install/update tests and packaging docs; kept `/codex-cloud-panel/` retirement guidance and Probe defaults unchanged |
| 2026-06-15 | probe-logs-db-retarget-panel-slim | Local verification passed for `v0.1.52`: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, WebUI tests/build, install script tests, diff check, rendered Probe QA, and compact regression tests covering quick_check/VACUUM plus scheduler app-server inactivity gating |
| 2026-06-15 | codex-path-auto-discovery | Local verification passed for `v0.1.53`: fmt, Rust workspace tests, clippy, WebUI tests/build, install script tests, and diff check. |
| 2026-06-15 | task-c-deploy-docs-auto-discovery | Updated deploy/script/docs surfaces for omitted Codex home auto-discovery, root/ubuntu systemd write paths, no-new-backup compact guidance, and cleanup only after health verification. |
| 2026-06-17 | v0.1.96-dual-entry-docs | Updated README, cloud runbook, Cloudflare Tunnel guide, optional helper script, and static install-script assertions for Linux/macOS/Tunnel acceptance boundaries. |
| 2026-06-17 | v0.1.97-macos-base-path-patch | Bumped patch versions to `0.1.97`, fixed local `/nexushub/` static WebUI routing, removed concrete `NexusHub-0.1.96-darwin-arm64.dmg` release workflow paths, and covered Cargo-derived macOS DMG versioning with install-script assertions. |
| 2026-06-17 | v0.1.98-mac-tauri-platform-split | Bumped patch versions to `0.1.98`, kept Tencent Cloud Linux WebUI as `https://661313.xyz/nexushub/`, moved macOS docs to Tauri App-only acceptance, deleted the Cloudflare Tunnel guide, and updated static docs checks to guard against Tunnel and macOS browser WebUI regressions. |
| 2026-06-17 | v0.1.99-cc-switch-style-native-tauri-alignment | Bumped versions to `0.1.99`, made macOS Tauri consume `webui/dist`, removed stale `desktop-ui`, added native desktop API coverage for the shared UI, retained release artifacts for Linux tarball, darwin app tarball, DMG, and sha256 files, and locally built `NexusHub_0.1.99_aarch64.dmg`. |
| 2026-06-18 | v0.1.100-macos-parity-probe-helper-ui-alignment | Bumped versions to `0.1.100`, fixed desktop runtime bridge coverage, fixed macOS Probe settings/events/logs-db parity, bundled and synced the local `nexushubd` helper for Probe Bark/Hook actions, and aligned desktop Ops/Probe/composer/right-inspector UI with the shared Linux WebUI surface. |
| 2026-06-18 | v0.1.102-unified-runtime-signed-updates | Bumped versions to `0.1.102`, added shared update service/status contracts, exposed one `NexusHub 更新` entry across Linux and macOS, added signed updater release artifacts (`nexushub-darwin-arm64.tar.gz.sig`, `latest.json`), and kept Cloudflare Turnstile login verification separate from updater signing. |
| 2026-06-18 | v0.1.102-cc-switch-architecture-alignment | Finished shared service convergence for threads/jobs/settings/system, routed Linux HTTP and macOS Tauri through thin adapters, removed frontend production use of `desktop_api_command` bridge/panel update aliases, and verified capability-matrix gating for Linux-only WebUI functions. |
| 2026-06-18 | v0.1.103-macos-updater-status-closure | Fixed the macOS updater status loop discovered during native App acceptance: signed updater `Check` results are now remembered by `desktop_update_status`, and the shared `NexusHub 更新` card can show the latest signed version without falling back to `unknown`. |
| 2026-06-18 | v0.1.104-cc-switch-architecture-closure | Closes the remaining cross-platform architecture gaps: service helpers are shared, macOS does not expose Web security/Turnstile commands, frontend domain APIs go through runtime transport, and component differences are capability-matrix driven. |
| 2026-06-19 | v0.1.107-cc-switch-runtime-rpc-closure | Closes the remaining runtime split: WebUI domain APIs dispatch to Linux `/api/rpc/:command`, macOS keeps typed Tauri commands, runtime no longer owns business route tables, Probe settings save normalization is shared in core, `CLAUDE.md` remains intentionally deleted, Probe desktop partial DTOs no longer white-screen, and stale older updater checks no longer show as available updates. |
| 2026-06-20 | v0.1.108-cc-switch-typed-rpc-closure | Closes the remaining typed RPC contract split: frontend domain APIs no longer maintain Web/Desktop route tables, Linux RPC accepts shared DTO wrappers and aliases, macOS typed commands cover shared domains without Web security entries, and static guards prevent route bridge regressions. |
| 2026-06-20 | v0.1.109-cc-switch-service-closure | Closes the remaining service split: Goal status/view planning and Linux update shell job specs are shared core services, frontend update prune is capability-driven, and macOS keeps Web security/admin/systemd/Nginx surfaces unavailable. |
| 2026-06-20 | v0.1.111-cc-switch-final-service-layer-closure | Closes the final cc-switch alignment gaps: upload validation/storage planning is shared in core, frontend domain APIs are split behind `runtime` transport, desktop compat `_command` wrappers stay retired, and macOS keeps Linux WebUI-only operations unavailable. |
| 2026-06-20 | v0.1.112-cc-switch-unified-architecture-backfill | Closes the remaining unified-architecture gaps: shared core plans thread steer/follow-up and desktop upload validation, cleanup capabilities are explicit, frontend mutations sit behind query/state facades, and Linux WebUI-only operations stay unavailable on macOS. |
