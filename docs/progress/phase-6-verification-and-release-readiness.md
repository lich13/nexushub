# Phase 6: Verification and Release Readiness

**Goal**: Verify repo readiness, release assets, and cloud deployment.<br>
**Status**: Complete

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
  - Notes: `v0.1.43` release exists at `https://github.com/lich13/nexushub/releases/tag/v0.1.43` with Linux tarball and checksum assets. Cloud deployment on `43.155.235.227` serves `https://661313.xyz/nexushub/`, listens on `127.0.0.1:15742`, and leaves legacy `codex-cloud-panel` on `127.0.0.1:15732`.

## Phase Notes

Remote, release, and cloud deploy are verified. The current finish line is released Linux handoff; macOS DMG and Windows ZIP/service packaging remain preview/planned.

## Phase Completion Checklist

- [x] All tasks above are checked off
- [x] MASTER.md phase count updated
- [x] MASTER.md "Current Status" updated to final state
