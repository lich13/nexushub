# Current status

- Delivered version: `1.1.4`; macOS App/helper and cloud webd upgraded from `1.1.3`.
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

- Release commit: `be7ada223c5815ca6e3c50c7a94af52d3b11ece9`. [CI 36281234627](https://github.com/lich13/nexushub/actions/runs/36281234627) passed frontend, backend and macOS Tauri checks. An existing menu test now waits for archived-thread selection before opening its menu; both browsers passed three consecutive targeted runs.
- [Release 36281467886](https://github.com/lich13/nexushub/actions/runs/36281467886) published [v1.1.4](https://github.com/lich13/nexushub/releases/tag/v1.1.4) with exactly seven assets. All seven GitHub SHA-256 digests, both sidecars, the updater signature and the sole `darwin-aarch64` mapping were verified. Source, Git objects and unpacked macOS/webd payloads passed privacy scanning.
- Official macOS installation matches all four release bundle files. Real sessions verified hidden metadata, AGENTS.md default collapse, manual expansion retained after refresh, Grok's 19-tool group with independently collapsed rows, and long-history scrolling. A real assistant reply copied without metadata. A Plan saved through the native App as a title-named UTF-8 Markdown file; all 5,180 bytes matched the clipboard, with no metadata or proposed-plan wrapper. Two inspected native session files retained their SHA-256.
- After closing the App, monitor remained running with zero TCP listening ports. Runtime helper matches the bundled helper. App SHA-256: `92a091eb28a3ea491ced152059812bf999144d8ebd8941ca18a330a74bab3e2f`; helper SHA-256: `c8345db3708e73f53a97b645a39cd1e2f1c1bb3bf2a4d1263f199e0fd66a4d07`.
- Authenticated HTTPS acceptance used the same real reply that previously exposed memory tags: display and the 810-byte clipboard result both omit metadata. Real Grok history retained 39 groups, including 34 mixed-tool groups; its AGENTS.md row started closed, expanded on request and stayed open after refresh. Outer page overflow was zero. No provider data was deleted or renamed during this acceptance.
- Cloud binary SHA-256 matches the release payload: `77e0627b050962a2ffe953d6fe6cac7de808c7e0918b60aa469070a17f070756`. Health is `{"ok":true}`, unauthenticated sensitive RPC returns 401, and systemd isolation remains intact. Config/env hashes and persistent notification/security settings were retained; the runtime monitor status and automatically re-encrypted Turnstile secret are excluded from ciphertext equality checks. The authenticated session remained usable.
- Packaging retains its existing linker ad-hoc macOS signature, without Apple Developer signing/notarization. The updater archive signature is independently verified; local installation preserved published bytes.

## Cleanup

Removed this task's root/Tauri build directories, renderer output and build-info file, browser artifacts, downloaded releases, unpacked payloads, temporary Plan export and old installed App. The DMG was detached. Cloud extraction, snippet staging and task recovery archive were removed after acceptance. Removed payloads total 4,145,970,582 logical bytes and 4,224,258,048 allocated bytes before the final small log cleanup; no task installation rollback remains. Dependencies, pre-existing generated schemas, native sessions, databases, credentials and production configuration remain.

Release payload SHA-256:

| Asset | SHA-256 |
| --- | --- |
| macOS DMG | `84f0cea31201e68497f488754af0465c047b8c26f8ff4df95b664ac2d7546973` |
| macOS updater archive | `c74695b81fb1dfdfc55b73e564210c5c7e4ee4581a6e82c045f880a3a8f48c5c` |
| Linux webd archive | `994e31a17aee03683bf3ccff4d9644ae69eea8cfc2253e12571e5b242f245644` |

Future evidence updates this document through normal commits; do not add historical Markdown or backups.
