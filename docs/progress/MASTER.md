# NexusHub Merge and Framework Expansion - Progress Tracker

> **Task**: Create a new NexusHub repo from codex-cloud-panel, preserve Codex behavior, merge Sentinel preview surfaces, and add a Claude Code read-only provider framework.
> **Started**: 2026-06-13
> **Last Updated**: 2026-06-13
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

**Active Phase**: Released Linux handoff complete<br>
**Active Task**: None  
**Blockers**: macOS launchd packaging and Windows Service packaging remain preview/planned; no blocker for Linux `/nexushub/`.

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
  last_updated: "2026-06-13"
```

## Task Telemetry Log

| Date | Task | Actual Effort | S.U.P.E.R Score | Unplanned Dependencies | Notes |
|:--|:--|:--|:--|--:|:--|
| 2026-06-13 | 1.1 | S | P/R pass | 0 | Added agent instruction surfaces |
| 2026-06-13 | 1.2 | M | P/E/R pass | 0 | Added spec docs and later updated them to released Linux state |
| 2026-06-13 | 1.3 | S | E/R pass | 0 | README DB path and preview scope aligned |
| 2026-06-13 | 2.2 | S | E/R pass | 0 | Cookie/upload/log/test fixture NexusHub rename with targeted tests |
| 2026-06-13 | 3.1-3.3 | M | S/P/R pass | 0 | Provider, Claude, Sentinel endpoints added in prior pass |
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
3. Do not replace legacy `/codex-cloud-panel/` until a separate migration task explicitly asks for that cutover.

## Session Log

| Date | Session | Summary |
|:--|:--|:--|
| 2026-06-13 | fresh-context continuation | Continued previous NexusHub migration, added governance docs, fixed stale non-migration names, prepared for full verification |
| 2026-06-13 | verification | Verified local handoff with Rust/WebUI/script checks; browser smoke blocked because no in-app Browser target was available |
| 2026-06-13 | release-deploy | Published `v0.1.43`, deployed `/nexushub/` to `43.155.235.227`, migrated admin/settings from the legacy panel DB, and verified both new and old services stay active on separate loopback ports |
