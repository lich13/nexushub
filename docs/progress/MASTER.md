# Current status

- Version: `1.1.2`.
- Scope: Goal and automatic-recovery retirement, unified provider running state, command execution groups, macOS/webd release simplification and database migration.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation state

- NexusHub Goal configuration, DTOs, RPC handlers, scheduler and recovery database columns are removed. Goal command names remain unavailable tombstones. Database opening drops `codex_thread_goals` and preserves Probe incidents.
- Codex, Grok and Pi use the shared running indicator. Consecutive command activity is grouped with native collapsible details; active and failed groups remain open.
- Command activity now uses two levels of native details: same-call events are paired into one command row, adjacent command rows form an outer group, and completed groups/rows start closed without resetting user toggles during polling.
- Codex, Grok and Pi assistant replies expose raw-Markdown copy buttons. All provider list rows support double-click rename with Enter/blur save and Escape cancel, subject to existing archive/activity protection.
- Linux Tauri packaging, AppImage/deb/rpm artifacts and xvfb desktop smoke are retired. CI contains frontend, backend and macOS Tauri jobs. Release stages are webd Linux, macOS ARM64 and aggregate publishing with seven assets.
- Deployment parameters remain explicit. No private host, domain, credential or real session is stored in Git.

## Fresh checks

- `cargo fmt --all` and workspace compilation pass after the Goal client split.
- `cargo test --workspace`: all current Rust tests pass, including Goal/config/database migration coverage.
- WebUI typecheck and unit tests pass, including execution-group and probe migration coverage.
- `bash scripts/test-install-script.sh` passes version, contract, privacy, six-document and release-boundary checks.
- Final local gates: WebUI 195 unit tests, Chromium/WebKit 82 browser tests, `cargo test --workspace`, Tauri tests and Clippy all pass; `python3 scripts/privacy-check.py --git-objects` and `git diff --check` pass.

## Acceptance boundary

Formal macOS acceptance is complete for `v1.1.2`: commit `60b0a8070c3756f9cf7ed9927dc2dd4cf3eba638`, CI run `36219211349`, Release run `36219375861`, and the [seven-asset release](https://github.com/lich13/nexushub/releases/tag/v1.1.2) are green. The downloaded DMG and webd tarball checksums matched their published `.sha256` files; `latest.json` is `1.1.2` and contains only `darwin-aarch64`. The installed macOS App and bundled helper report `1.1.2`.

The installed Tauri entrypoint exposed Codex, Grok Build, Pi, Probe and Settings. A real Grok session rendered native `Execute` records as collapsed command groups, including groups of 2, 6, 1 and 4 commands. Pi showed the explicit empty state because no local Pi session was discovered. After closing the App, the managed Probe monitor remained running and TCP port `15742` had no listener. The local NexusHub database no longer contains `codex_thread_goals`; `probe_error_incidents` retains error/delivery columns only. Historical Probe rows that predate this release remain read-only evidence; no new Goal scheduler, RPC or recovery path is active.

The current working-tree frontend was also built into the installed `1.1.2` App without changing version or Release assets. The real Codex entry showed a two-level execution group with one row per command and reply copy controls; Grok showed grouped command records and reply copy controls; Pi showed the explicit empty state. The App was closed again while the monitor stayed running and port `15742` remained unbound.

Cloud deployment was not run in this checkout because `scripts/deploy-cloud.sh` requires explicit production `HOST`, domain and archive inputs. No deployment value was guessed or written to Git. Authenticated cloud acceptance therefore remains an external deployment step; cloud hosts without Pi should use the documented empty-state and isolated interface checks. Linux desktop acceptance is intentionally retired.

Future evidence is appended to this document through normal commits. Do not add another plan, summary or archive Markdown.
