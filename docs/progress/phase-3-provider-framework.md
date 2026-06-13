# Phase 3: Provider Framework

**Goal**: Make Codex, Claude Code, Sentinel, and future CLI providers visible through typed preview surfaces.  
**Status**: Complete

## Tasks

- [x] **Task 3.1**: Add provider registry and API endpoints
  - Priority: P0
  - Effort: M
  - Test Expectation: Rust provider tests and WebUI API tests.
  - Memory Impact: None.
  - Acceptance: `/api/providers`, `/api/platform`, `/api/plugins` return typed data.
  - Notes: Completed in prior pass.

- [x] **Task 3.2**: Add read-only Claude Code discovery
  - Priority: P0
  - Effort: M
  - Test Expectation: Rust tests for discovery/redaction.
  - Memory Impact: Instruction rule documented.
  - Acceptance: `~/.claude/projects` and redacted settings are visible without writes.
  - Notes: Completed in prior pass.

- [x] **Task 3.3**: Add Sentinel preview status
  - Priority: P1
  - Effort: S
  - Test Expectation: Rust provider tests and WebUI API tests.
  - Memory Impact: None.
  - Acceptance: Hook/Bark/log maintenance status is exposed without hidden control.
  - Notes: Completed in prior pass.

## Phase Notes

Cursor and Gemini are registry placeholders only.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to next phase

