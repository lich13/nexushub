# Phase 6: Verification and Release Readiness

**Goal**: Verify repo readiness, release assets, and cloud deployment.<br>
**Status**: Complete

> Historical snapshot: this phase file records the original `v0.1.43` Linux
> release readiness milestone. Current release/deploy acceptance is tracked in
> `docs/progress/MASTER.md` under the latest `v0.1.126` cc-switch
> release/deploy guard closure matrix.

## Tasks

- [x] **Task 6.1**: Run full local verification
  - Priority: P0
  - Effort: M
  - Test Expectation: Full Rust/WebUI/script commands.
  - Memory Impact: Progress telemetry updated.
  - Acceptance: Formatting, tests, clippy, WebUI test/build, install-script test pass or failures are documented.
  - Notes: Passed `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `corepack pnpm@11.0.8 --dir webui test`, `corepack pnpm@11.0.8 --dir webui build`, and `bash scripts/test-install-script.sh`. Current logged-in rendered acceptance uses Chrome 插件验收.

- [x] **Task 6.2**: Document release/deploy state
  - Priority: P0
  - Effort: S
  - Test Expectation: Docs-only.
  - Memory Impact: Progress surface updated.
  - Acceptance: GitHub release assets and cloud deployment evidence are recorded after verification.
  - Notes: Historical `v0.1.43` release exists at `https://github.com/lich13/nexushub/releases/tag/v0.1.43` with Linux tarball and checksum assets. Current release/deploy status is maintained in `MASTER.md`; do not treat this old phase note as the active cloud state.

## Phase Notes

This file is retained as a historical phase record. Current remote, release,
Linux cloud deploy, and macOS Tauri App acceptance evidence belongs in
`MASTER.md` and the latest release tag notes.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to final state
