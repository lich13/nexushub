# Phase 2: Codex Non-Regression

**Goal**: Preserve full Codex behavior while renaming local product surfaces.  
**Status**: Complete

## Tasks

- [x] **Task 2.1**: Keep bridge-first create/send/stop/thread behavior
  - Priority: P0
  - Effort: L
  - Test Expectation: `cargo test --workspace`; future live bridge smoke.
  - Memory Impact: None.
  - Acceptance: Existing Codex endpoints remain compatibility wrappers and bridge fallback semantics are unchanged.
  - Notes: Full Rust workspace tests passed on 2026-06-13.

- [x] **Task 2.2**: Remove non-migration stale product names
  - Priority: P0
  - Effort: S
  - Test Expectation: Targeted Rust tests and stale-string search.
  - Memory Impact: None.
  - Acceptance: Cookie, upload runtime path, log branding, and fixtures use NexusHub names.
  - Notes: Completed 2026-06-13.

- [x] **Task 2.3**: Preserve official state DB and rollout read model
  - Priority: P0
  - Effort: M
  - Test Expectation: Rust Codex tests and existing thread parsing tests.
  - Memory Impact: None.
  - Acceptance: No official Codex DB schema mutation is introduced.
  - Notes: Full Rust workspace tests passed on 2026-06-13.

## Phase Notes

Legacy `codex-cloud-panel` strings in install/update migration tables and tests are intentional compatibility inputs. Existing `/home/ubuntu/codex-admin/bin/codex-cloud-*` wrappers belong to the cloud Codex chain and are intentionally preserved.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to next phase
