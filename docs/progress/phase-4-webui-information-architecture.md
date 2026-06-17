# Phase 4: WebUI Information Architecture

**Goal**: Surface the new provider framework without changing the existing visual system.  
**Status**: Complete

## Tasks

- [x] **Task 4.1**: Extend navigation for Claude, Probe, plugins, ops previews
  - Priority: P0
  - Effort: M
  - Test Expectation: WebUI tests, typecheck/build.
  - Memory Impact: None.
  - Acceptance: Desktop and mobile nav can reach all preview pages.
  - Notes: Completed in prior pass.

- [x] **Task 4.2**: Keep Codex chat non-regressed
  - Priority: P0
  - Effort: L
  - Test Expectation: Existing WebUI tests and Chrome 插件验收 for logged-in flows.
  - Memory Impact: None.
  - Acceptance: Thread list, detail, SSE, Plan/Questions, upload, stop/follow-up remain available.
  - Notes: Existing chat route remains the primary workspace.

- [x] **Task 4.3**: Add preview pages for files/Git/terminal as disabled/planned entries
  - Priority: P2
  - Effort: S
  - Test Expectation: Typecheck/build.
  - Memory Impact: None.
  - Acceptance: UI shows planned provider tooling without exposing shell jobs.
  - Notes: Completed in prior pass.

## Phase Notes

No router was introduced; the current `View`-based shell was extended.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to next phase
