# Current status

- Target version: `1.1.4`; installed macOS and cloud baseline: `1.1.3`.
- Scope: hide structured memory metadata in display/copy/export, collapse AGENTS.md file activities and body sections, and deliver the macOS/webd patch release.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation state

- Shared Markdown cleaning removes structured memory wrappers, citation entries, rollout IDs and standalone memory citation fields. Code examples and ordinary memory references remain. Rendering, reply copy, Plan copy/download and filenames use the cleaned source; native sessions remain untouched.
- AGENTS.md activities and body sections default to closed, including active or failed tools. Instruction-only groups start closed; mixed groups keep their existing rules. Stable scoped disclosure choices survive polling, streaming text, theme changes and mobile list/detail transitions. Body summaries expose a basename and line/byte counts.
- Grok still groups adjacent native tools with paired calls/results and independent child disclosures. Codex and Pi retain command groups. Replies retain copy controls, Plans retain title-named Markdown downloads, and provider rows retain protected double-click rename and batch management.
- NexusHub Goal management and recovery remain retired. Probe error monitoring, encrypted Bark delivery, native sessions and configuration retain their existing behavior.
- CI retains frontend, backend and macOS Tauri jobs. Release retains seven macOS ARM64/updater and Linux webd assets, with only `darwin-aarch64` in the updater manifest. Linux desktop remains retired.
- Deployment inputs and private acceptance evidence stay outside Git.

## Checks and acceptance

Local gates passed: WebUI frozen install/typecheck, 213 unit tests, both builds and 102 Chromium/WebKit cases; 546 workspace Rust tests and 64 Tauri tests; both format/Clippy gates, contract/install boundaries, privacy scanning and diff checks. Browser checks were rerun against a quiescent source tree after development hot reload interrupted the initial full run.

Matching CI/Release, official macOS installation, authenticated cloud acceptance and task cleanup remain pending.

Future evidence updates this document through normal commits; do not add historical Markdown or backups.
