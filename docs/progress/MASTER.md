# Current status

- Target: `1.2.1`, implementation and local checks complete; final release and installed-App acceptance pending.
- API migration to `1.2.0` passed public and retained-data checks. Final delivery target is `1.2.1`: native Plan saving fixes a WebKit download stall discovered in installed-App acceptance.
- Scope: remove cloud WebUI/login/Turnstile, use an independent administrator API Key and native Keychain connection, add 本机/腾讯云 switching with isolated data and operations.
- Preserve: Codex/Grok/Pi reading and management, native attachments, question bubbles, tool timeline, memory filtering, AGENTS.md folding, Plan export, file operations and both machines' independent Bark/monitor behavior.
- Local checks: root Rust 563 full-suite tests plus three added targeted regressions, Tauri 71 tests, WebUI 256 tests and 174 Chromium/WebKit cases; formatting, Clippy, frozen install, typecheck, desktop build, contract and privacy checks passed. The native Plan correction additionally passed 25 targeted unit tests and 22 Plan/connection browser cases. Fixtures are not production acceptance.
- Required before delivery: frontend/backend/macOS Tauri gates, matching CI, seven signed/checksummed release assets, official App installation and API deployment, real target-switching acceptance, public retirement checks and precise cleanup.
- No production credentials, deployment values or real session contents belong in this document. Normal incremental Git history continues.
