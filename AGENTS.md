# Agent Instructions

NexusHub is a Rust + React operations console built from the codex-cloud-panel base. Treat this repository as the new product surface and keep the original source repositories untouched unless the user explicitly asks otherwise.

## Required Context

- Read `README.md`, `DESIGN.md`, and `docs/progress/MASTER.md` before non-trivial edits.
- If `docs/progress/MASTER.md` exists, resume from its current phase instead of restarting the plan.
- Current upstream is `https://github.com/lich13/nexushub`.
- The production Linux deployment is `https://661313.xyz/nexushub/` on `43.155.235.227`; the legacy `codex-cloud-panel` path remains separate.

## Safety Boundaries

- Preserve the local app-server bridge boundary. The WebUI must never expose the root Codex app-server socket, `/v1`, `/responses`, or metrics publicly.
- Keep Codex state reads based on official Codex state DB, rollout files, session indexes, and app-server bridge responses. Do not mutate official Codex database schemas.
- Use the app-server bridge first for Codex create/send/stop/thread behavior. `codex exec --json` is fallback only.
- Do not add arbitrary shell execution to the WebUI. Maintenance actions must be fixed jobs with authorization, CSRF protection, audit records, output redaction, and failure classification.
- Archive and hidden-thread deletion require dry-run visibility plus button confirmation.
- Do not add a WebUI network-access checkbox. Generated sandbox policies default network access to enabled.
- Claude Code provider work is read-only in V1: project/session/settings discovery only. Do not write `~/.claude` or launch/resume/send/stop Claude sessions unless a later task explicitly adds that feature with tests.
- Sentinel V1 is observation and maintenance oriented. Do not add hidden desktop control, automatic recovery, or competing app-server probes unless the bridge is unavailable and the behavior is explicitly scoped.

## Verification

Use these commands for normal handoff:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build
bash scripts/test-install-script.sh
```

`bash scripts/package-linux.sh` is canonical only on Linux x86_64. `ALLOW_HOST_MISMATCH=1` is for local smoke archives only.
