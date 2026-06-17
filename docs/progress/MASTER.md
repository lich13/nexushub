# NexusHub Merge and Framework Expansion - Progress Tracker

> **Task**: Continue NexusHub from the codex-cloud-panel base, preserve Codex behavior, replace the cloud Sentinel runtime with built-in Probe surfaces, and keep the Claude Code provider read-only.
> **Started**: 2026-06-13
> **Last Updated**: 2026-06-17
> **Mode**: RELEASED_LINUX

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

**Active Phase**: v0.1.94 Ops cleanup and archive cleanup split<br>
**Active Task**: `v0.1.94` removes obsolete Codex/Claude maintenance actions from the Ops page and API surface, splits archive cleanup into archived-thread and hidden-thread panels, and keeps rendered WebUI acceptance on Chrome 插件验收.
**Blockers**: None. Current rendered WebUI acceptance requires Chrome 插件验收 for logged-in QA.

## Governance Status

**Shared instruction surface**: `AGENTS.md`  
**Claude Code instruction surface**: `CLAUDE.md`  
**Other platform rule surfaces**: none detected  
**Memory surface**: native memory only  
**Memory fallback path**: none

## Adaptive Control State

```yaml
adaptive:
  mode: RELEASED_LINUX
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
  last_updated: "2026-06-15"
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
```

## Next Steps

1. Keep `origin` pointed at `https://github.com/lich13/nexushub`.
2. For every future release, wait for CI and Release workflows, verify release assets, deploy to `43.155.235.227`, and smoke `https://661313.xyz/nexushub/`.
3. Keep the retired legacy `/codex-cloud-panel/` path returning `404`; NexusHub is the public panel surface under `/nexushub/`.

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
