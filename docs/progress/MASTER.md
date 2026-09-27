# Current status

- Target version: `1.1.5`; implementation, release, installation and deployment are complete.
- Scope: add Codex ordinary-feedback Bark notifications, source-path Markdown copy/Finder actions, and the merged `更新与维护` settings surface while retaining the 1.1.4 display/export behavior.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation state

- Shared Markdown cleaning removes structured memory wrappers, citation entries, rollout IDs and standalone memory citation fields. Code examples and ordinary memory references remain. Rendering, reply copy, Plan copy/download and filenames use the cleaned source; native sessions remain untouched.
- AGENTS.md activities and body sections default to closed, including active or failed tools. Instruction-only groups start closed; mixed groups keep their existing rules. Stable scoped disclosure choices survive polling, streaming text, theme changes and mobile list/detail transitions. Body summaries expose a basename and line/byte counts.
- Grok still groups adjacent native tools with paired calls/results and independent child disclosures. Codex and Pi retain command groups. Replies retain copy controls, Plans retain title-named Markdown downloads, and provider rows retain protected double-click rename and batch management.
- NexusHub Goal management and recovery remain retired. `system.status` is unavailable and `system.capabilities` is used for bootstrap. Codex ordinary final feedback requests reuse `reply_needed` Bark only after verified terminal evidence; repeated hooks and monitor scans are deduplicated. Probe error monitoring, encrypted Bark delivery, native sessions and configuration retain their existing behavior.
- CI retains frontend, backend and macOS Tauri jobs. Release retains seven macOS ARM64/updater and Linux webd assets, with only `darwin-aarch64` in the updater manifest. Linux desktop remains retired.
- Deployment inputs and private acceptance evidence stay outside Git.

## Checks and acceptance

Earlier 1.1.4 release evidence remains below as historical context. Current 1.1.5 local gates passed: WebUI frozen install/typecheck/build, 236 unit tests and 108 Chromium/WebKit cases; workspace and Tauri Rust tests, format/Clippy gates, contract/install boundaries, privacy scanning and diff checks. The local Tauri packaging cache was not reusable because of stale process-macro artifacts; the official macOS and Linux webd assets were built successfully by the release workflow.

- Implementation commit: `9d67da7e049b7c8652e6232114b2060efc62b81a`. [CI 36290750431](https://github.com/lich13/nexushub/actions/runs/36290750431) passed frontend, backend and macOS Tauri checks. [Release 36291024532](https://github.com/lich13/nexushub/actions/runs/36291024532) published [v1.1.5](https://github.com/lich13/nexushub/releases/tag/v1.1.5) with seven assets; `latest.json` contains only `darwin-aarch64`.
- The seven release asset digests, checksum sidecars and updater Minisign signature were independently verified. The unpacked macOS app and Linux webd payloads passed the privacy scan (329/344 checked items, zero failures); Git object privacy scan covered 825 objects with zero failures.
- The official macOS App is installed at version `1.1.5`; its bundled helper reports `nexushub-webd 1.1.5` and has SHA-256 `81b1c05987246462cd610ad62a3f432226bd9dbfea12fd5bb6c7eca2f2ec63e4`. A real session verified settings merged into `更新与维护` plus `账户与安全`, no `系统状态`, path copy to the exact source path, and Finder reveal with the target Markdown file selected. Memory metadata remained hidden, AGENTS.md content stayed collapsed until expanded, and existing reply/Plan controls remained available.
- After closing the App, the monitor process remained running and `lsof` reported zero NexusHub TCP listeners. The installed helper matched the release payload. macOS keeps its existing linker ad-hoc signature; the updater archive signature is independently verified, without Apple Developer notarization.
- Authenticated HTTPS acceptance verified the merged settings surface, Grok tool grouping, hidden metadata and AGENTS.md disclosure behavior. Web path controls remained path buttons rather than navigation; browser regression tests cover copying the server path. The live browser clipboard API was not independently observable in the isolated CUA context, so this point is backed by the 108-case Chromium/WebKit suite and DOM/path-control evidence.
- Cloud version is `1.1.5`; the deployed webd binary SHA-256 is `c8b4eb2823bc963fe07b9d570fef96276205b1c041b0fddddd7d4a4ed8dc31fc`, health is `{"ok":true}`, and systemd hardening remains `ProtectHome=read-only`/`ProtectSystem=full` with the existing provider write allowlist. Persistent sessions, credentials, notification/security settings and configuration were retained.
- Bark transport was tested through the existing settings flow and the user confirmed receipt of the test notification. The deterministic ordinary-feedback classifier and monitor delivery/deduplication tests passed; a separate fresh production turn deliberately asking for feedback was not generated during this run.

## 1.1.5 final evidence

The 1.1.5 implementation, matching CI, seven-asset Release, official macOS installation, cloud deployment and real settings/path acceptance are complete. The task-level cloud recovery archive was removed after acceptance. Remaining cleanup is limited to the exact temporary task root; formal App, monitor, cloud service, dependencies, credentials, configuration, databases and native sessions remain in place.

## Earlier 1.1.4 cleanup record

Removed this task's root/Tauri build directories, renderer output and build-info file, browser artifacts, downloaded releases, unpacked payloads, temporary Plan export and old installed App. The DMG was detached. Cloud extraction, snippet staging and task recovery archive were removed after acceptance. Removed payloads total 4,145,970,582 logical bytes and 4,224,258,048 allocated bytes before the final small log cleanup; no task installation rollback remains. Dependencies, pre-existing generated schemas, native sessions, databases, credentials and production configuration remain.

Earlier 1.1.4 release payload SHA-256:

| Asset | SHA-256 |
| --- | --- |
| macOS DMG | `84f0cea31201e68497f488754af0465c047b8c26f8ff4df95b664ac2d7546973` |
| macOS updater archive | `c74695b81fb1dfdfc55b73e564210c5c7e4ee4581a6e82c045f880a3a8f48c5c` |
| Linux webd archive | `994e31a17aee03683bf3ccff4d9644ae69eea8cfc2253e12571e5b242f245644` |

Future evidence updates this document through normal commits; do not add historical Markdown or backups.
