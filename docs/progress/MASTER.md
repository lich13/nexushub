# Current status

- Version: `1.1.2`.
- Scope: Goal and automatic-recovery retirement, unified provider running state, command execution groups, macOS/webd release simplification and database migration.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation state

- NexusHub Goal configuration, DTOs, RPC handlers, scheduler and recovery database columns are removed. Goal command names remain unavailable tombstones. Database opening drops `codex_thread_goals` and preserves Probe incidents.
- Codex, Grok and Pi use the shared running indicator. Consecutive command activity is grouped with native collapsible details; active and failed groups remain open.
- Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired. CI contains frontend, backend and macOS Tauri jobs. Release stages are webd Linux, macOS ARM64 and aggregate publishing with seven assets.
- Deployment parameters remain explicit. No private host, domain, credential or real session is stored in Git.

## Fresh checks

- `cargo fmt --all` and workspace compilation pass after the Goal client split.
- `cargo test --workspace`: all current Rust tests pass, including Goal/config/database migration coverage.
- WebUI typecheck and unit tests pass, including execution-group and probe migration coverage.
- `bash scripts/test-install-script.sh` passes version, contract, privacy, six-document and release-boundary checks.

## Acceptance boundary

Formal macOS installation, signed release assets, matching GitHub CI/Release, cloud deployment and authenticated Browser checks must be recorded here after the 1.1.2 commit and tag exist. Linux desktop acceptance is intentionally retired. Cloud hosts without Pi use empty-state and isolated interface checks only.

Future evidence is appended to this document through normal commits. Do not add another plan, summary or archive Markdown.
