# Task Breakdown

## Overview

- **Total Phases**: 6
- **Total Tasks**: 16
- **Estimated Total Effort**: XL
- **Tracking Mode**: GitHub release plus cloud deployment verified

## S.U.P.E.R Design Constraints

- **S (Single Purpose)**: New provider modules must own one provider-specific concern. Do not add unrelated logic to `api.rs`, `codex.rs`, or `App.tsx` without a follow-up split plan.
- **U (Unidirectional Flow)**: Preserve API -> core -> provider/service flow. UI code calls typed API helpers rather than raw fetches from components.
- **P (Ports over Implementation)**: Provider work must expose serializable structs and stable route contracts before control actions are added.
- **E (Environment-Agnostic)**: Linux production paths belong in config/defaults; macOS and Windows support remains preview until service installers are verified.
- **R (Replaceable Parts)**: Codex, Claude Code, built-in Probe, and future providers must remain independently replaceable behind registry/API boundaries.

## Testing and Governance Constraints

- Feature work, behavior changes, API contracts, parsing, permissions, jobs, deploy scripts, and persistence changes require automated tests or a documented no-test rationale.
- Changes that affect future agent behavior must update `AGENTS.md` or `CLAUDE.md`.
- Durable project rules stay in native memory unless the user explicitly selects a repo-local fallback.

## Phase 1: Governance Baseline

**Goal**: Establish NexusHub identity, safety rules, and release/deploy continuity.
**Prerequisite**: Fresh NexusHub repo exists.
**S.U.P.E.R Focus**: P, E, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 1.1 | Add `AGENTS.md` and `CLAUDE.md` | P0 | S | - | A | P, R | Docs-only; verify files and final commands | Instruction surfaces updated | Shared and Claude-specific rules document bridge, safety, Claude read-only, built-in Probe, and verification commands |
| 1.2 | Create analysis, plan, and progress docs | P0 | M | 1.1 | A | P, E | Docs-only; verify links and files | Progress surface updated | Required spec-driven docs exist and `docs/progress/MASTER.md` records current release/deploy state |
| 1.3 | Align README/runbook with NexusHub scope | P1 | S | - | B | E, R | Docs-only; run stale-string and command checks | None | README describes provider previews, `/opt/nexushub/nexushub.sqlite`, and Linux package caveats |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 1.1, 1.2 | M | Low | `AGENTS.md`, `CLAUDE.md`, `docs/` |
| B | 1.3 | S | Low | `README.md`, `docs/cloud-deploy-runbook.md` |

## Phase 2: Codex Non-Regression

**Goal**: Preserve full Codex behavior while renaming local product surfaces.
**Prerequisite**: Phase 1.
**S.U.P.E.R Focus**: U, P, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 2.1 | Keep bridge-first create/send/stop/thread behavior | P0 | L | 1.2 | A | U, R | `cargo test --workspace`; future live bridge smoke | None | Existing Codex endpoints remain compatibility wrappers and bridge fallback semantics are unchanged |
| 2.2 | Remove non-migration stale product names | P0 | S | 1.2 | B | E, R | Add targeted Rust tests where applicable; run stale-string search | None | Cookie, upload runtime path, log branding, and fixtures use NexusHub names |
| 2.3 | Preserve official state DB and rollout read model | P0 | M | 2.1 | A | P, R | Rust Codex tests and existing thread parsing tests | None | No official Codex DB schema mutation is introduced |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 2.1, 2.3 | L | Medium | `codex.rs`, `api.rs`, bridge tests |
| B | 2.2 | S | Low | `auth.rs`, `uploads.rs`, `main.rs`, tests |

## Phase 3: Provider Framework

**Goal**: Make Codex, Claude Code, built-in Probe, and future CLI providers visible through typed surfaces.
**Prerequisite**: Phase 2.
**S.U.P.E.R Focus**: S, P, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 3.1 | Add provider registry and API endpoints | P0 | M | 2.1 | A | S, P, R | Rust provider tests and WebUI API tests | None | `/api/providers`, `/api/platform`, `/api/plugins` return typed data |
| 3.2 | Add read-only Claude Code discovery | P0 | M | 3.1 | B | S, P, E | Rust tests for discovery/redaction | Instruction rule already documented | `~/.claude/projects` and redacted settings are visible without writes |
| 3.3 | Add built-in Probe status | P1 | S | 3.1 | C | S, P, E | Rust provider tests and WebUI API tests | None | Hook/Bark/log maintenance status is exposed without hidden control |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 3.1 | M | Medium | `providers.rs`, `api.rs`, `types.ts`, `api.ts` |
| B | 3.2 | M | Low | `claude_code.rs`, Claude page |
| C | 3.3 | S | Low | `probe.rs`, Probe page |

## Phase 4: WebUI Information Architecture

**Goal**: Surface the new provider framework without changing the existing visual system.
**Prerequisite**: Phase 3.
**S.U.P.E.R Focus**: S, P, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 4.1 | Extend navigation for Claude, Probe, plugins, ops previews | P0 | M | 3.1 | A | S, R | WebUI tests, typecheck/build | None | Desktop and mobile nav can reach all preview pages |
| 4.2 | Keep Codex chat non-regressed | P0 | L | 4.1 | A | U, R | Existing WebUI tests and manual browser smoke in future | None | Thread list, detail, SSE, Plan/Questions, upload, stop/follow-up remain available |
| 4.3 | Add preview pages for files/Git/terminal as disabled/planned entries | P2 | S | 4.1 | B | P, R | Typecheck/build | None | UI shows planned provider tooling without exposing shell jobs |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 4.1, 4.2 | L | High | `webui/src/App.tsx` |
| B | 4.3 | S | Medium | `webui/src/App.tsx` |

## Phase 5: Three-Platform Service Model

**Goal**: Keep Linux real and document macOS/Windows preview path support.
**Prerequisite**: Phase 3.
**S.U.P.E.R Focus**: E, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 5.1 | Implement `PlatformPaths` for Linux/macOS/Windows | P0 | S | 3.1 | A | E, R | Rust platform tests | None | Paths match `/opt/nexushub`, `~/Library/Application Support/NexusHub`, and `%ProgramData%\\NexusHub` |
| 5.2 | Harden Linux install/update migration | P0 | M | 5.1 | A | E, R | `bash scripts/test-install-script.sh` | None | Legacy paths migrate to `/opt/nexushub` and fixed wrappers are installed |
| 5.3 | Mark Windows/macOS packaging as preview until verified | P1 | S | 5.1 | B | E | Docs-only; README review | None | Docs do not overclaim unverified DMG/ZIP/service installers |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 5.1, 5.2 | M | Medium | `platform.rs`, deploy scripts |
| B | 5.3 | S | Low | `README.md`, docs |

## Phase 6: Verification and Release Readiness

**Goal**: Verify repo readiness, release assets, and cloud deployment.
**Prerequisite**: Phases 1-5.
**S.U.P.E.R Focus**: P, E, R

| # | Task | Priority | Effort | Depends On | Lane | S.U.P.E.R | Test Expectation | Memory Impact | Acceptance Criteria |
|:--|:--|:--|:--|:--|:--|:--|:--|:--|:--|
| 6.1 | Run full local verification | P0 | M | 1-5 | A | P, R | Full Rust/WebUI/script commands | Progress telemetry updated | Formatting, tests, clippy, WebUI test/build, install-script test pass or failures are documented |
| 6.2 | Document release/deploy state | P0 | S | 6.1 | A | E, R | Docs-only | Progress surface updated | GitHub release assets and cloud deployment evidence are recorded after verification |

### Parallel Lanes

| Lane | Tasks | Combined Effort | Merge Risk | Key Files |
|:--|:--|:--|:--|:--|
| A | 6.1, 6.2 | M | Low | progress docs, final verification |
