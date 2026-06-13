# Phase 5: Three-Platform Service Model

**Goal**: Keep Linux real and document macOS/Windows preview path support.  
**Status**: Complete

## Tasks

- [x] **Task 5.1**: Implement `PlatformPaths` for Linux/macOS/Windows
  - Priority: P0
  - Effort: S
  - Test Expectation: Rust platform tests.
  - Memory Impact: None.
  - Acceptance: Paths match `/opt/nexushub`, `~/Library/Application Support/NexusHub`, and `%ProgramData%\\NexusHub`.
  - Notes: Completed in prior pass.

- [x] **Task 5.2**: Harden Linux install/update migration
  - Priority: P0
  - Effort: M
  - Test Expectation: `bash scripts/test-install-script.sh`.
  - Memory Impact: None.
  - Acceptance: Legacy paths migrate to `/opt/nexushub` and fixed wrappers are installed.
  - Notes: Completed in prior pass; full verification still reruns script test.

- [x] **Task 5.3**: Mark Windows/macOS packaging as preview until verified
  - Priority: P1
  - Effort: S
  - Test Expectation: Docs-only; README review.
  - Memory Impact: None.
  - Acceptance: Docs do not overclaim unverified DMG/ZIP/service installers.
  - Notes: README marks Windows packaging as preview/planned.

## Phase Notes

Linux systemd is the only verified deployment service in this local handoff.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to next phase

