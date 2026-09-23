# Architecture and development

## Ownership

Two host surfaces share one WebUI and core: `desktop_embedded_tauri` and `linux_server_webui`. Components -> query/domain/runtime -> typed transport -> `NexusHubUseCases` -> provider/read model or controlled effects. HTTP handlers stay under api domains and Tauri commands/services remain thin.

| Area | Responsibility |
| --- | --- |
| Core Codex | Official state/index/rollout reads, identity, native naming, archive plans and restricted Goal protocol |
| Core Grok / Pi | Native session discovery, history, identity and activity checks, narrow rename and confirmed local deletion |
| Read cache | Bounded snapshots invalidated by size, inode, mtime and ctime |
| Probe / monitor | Canonical main-task notification selection, dedupe, redaction, cursors and restricted recovery |
| Services / DTOs | Shared contracts and plans for threads, providers, settings, cleanup, jobs, updates and security |
| Database / config / platform | NexusHub settings/events, persistent paths and preserved compatibility rows |
| WebUI | Shared visual vocabulary; query hooks own requests/cache effects; domain helpers own pure view models |
| Deploy and packaging | Native artifacts, exact-tag install, signed desktop updater and service isolation |

Codex notifications use the canonical main turn's final answer, never intermediate/tool/internal text. Unresolved questions retain call identity and confirmation; answered questions do not notify again. Bark and restricted recovery switches are independent. Recovery only reactivates eligible blocked/usageLimited/budgetLimited Goals, never paused/complete Goals or a turn. macOS uses its existing fixed monitor LaunchAgent; Linux uses the server process.

The current contract registry, not historical matrices, defines the public API. Task creation, sending, stop/steer/fork, uploads, followup execution, question/plan/approval actions, manual Goal RPC and desktop LAN WebUI are retired. Preserve historical database rows without execution or fallback.

## Changing a shared action

1. Run `node scripts/contract-next-action-checklist.mjs <action-id>`; for a new action run the global checklist.
2. Update `contracts/nexushub-contract.json` and its schema: action ID, scope, kind, coreUseCase, Linux RPC, Tauri command, WebUI wrapper, dtoOwner and request/response DTO. Host-only actions need a reason and must not enter the public allowlist.
3. Implement the core use case and DTO/plan. Register DTO ownership in `contract_dtos.rs`.
4. Add the WebUI wrapper, query hook, domain behavior and `contractDtoMap.ts` marker. Keep components independent of raw API/runtime/cache calls.
5. Add thin transport adapters, invoke registration and dispatcher mappings. The public route stays `/api/rpc/:command`; Pi uses `sessionKey` for file identity while `id` remains the native copied ID.
6. Run contract/schema, Rust/WebUI/Tauri parity guards and install-script checks, then Browser acceptance on authenticated Linux WebUI and Computer Use acceptance on the official macOS App.

Provider reads must have no side effects. Pi uses the native JSONL tree's current leaf and ancestor chain; metadata naming is taken from the latest session_info record. Native ID need not be a UUID. Different files can share an ID, so file identity cannot be inferred from it. Resolve an opaque sessionKey only against discovered files inside the configured root. Invalid/incomplete or unknown-format histories cannot be mutated. Read-only legacy conversion remains in memory.

Pi native rename issues only get_state, set_session_name and get_state over bounded stdio, validates the exact file/ID/name and reaps the child. Provider extensions/tools are disabled. Unknown activity, missing CLI or unsafe storage yields an explicit disabled reason; no guessed inactive state and no local rename fallback. Confirmed Pi deletion removes one JSONL; Grok removes one session directory. The parent workspace is never a deletion target.

## Platform and release boundary

`cc-switch origin/main` is only a reference for desktop Tauri packaging. `cc-switch feat/webd` is a distinct experimental headless/FHS reference, not a replacement dispatcher architecture. Windows desktop and Linux arm64 remain outside the accepted release matrix.

Release has three native build lines: macOS ARM64; Linux Tauri x86_64 (`NexusHub-*-Linux-x86_64.AppImage`, deb/rpm); and Tencent headless `nexushub-webd-linux-x86_64.tar.gz`. The updater manifest contains only desktop archives and their signatures. Desktop helper injection must restore its source placeholder. Server and desktop frontend bases are built separately. Linux desktop WebKit/GTK and xvfb dependencies do not belong on Tencent.

## Verification and maintenance

Root Rust and the independent Tauri workspace each run fmt, tests and Clippy; WebUI runs typecheck, unit tests, both builds and Chromium/WebKit regressions. Install-script guards verify deployment, six current Markdown files, links, contracts and asset boundaries. Native/production smoke uses isolated disposable sessions and preserves user data. Test timeout fixtures resolve their fake executable directly and isolate process discovery; do not weaken native validation to fix a fixture race.

Fresh release evidence must tie the implementation SHA, CI, tag, assets and installed versions together. API fixtures prove adapter behavior; installed App/authenticated Browser interactions prove user entrypoints. Linux AppImage smoke runs under xvfb. Preserve original runtime/configuration before replacement, restore on failure, and clean only identified task-created artifacts after acceptance.

Future edits update these six current documents. Version 1.0.0 starts a new repository history; subsequent changes use normal commits.

## Batch and native notifications

`sessions.bulkPreview`/`sessions.bulkExecute` enter the shared facade and accept at most 100 explicit provider keys. Codex archived deletion uses a SQLite transaction, file quarantine, reference checks and atomic index replacement; errors restore the original state or report the retained recovery path. Grok/Pi reuse their native scoped deletion guards.

`native_probe` resolves provider-specific identities and terminal turns, `db/native_probe` owns persistent baselines/cursors/delivery claims, and `provider_monitor` shares the encrypted Bark sender across cloud and the port-free desktop helper. First enable creates a baseline. Re-enabling skips history. Confirmed deliveries do not repeat after restart; an interrupted uncertain delivery is recorded without an automatic resend. Pi has no reliable final-failure marker without extensions.

Native Codex log maintenance has no callable entry or scheduler. Config loading removes its old section and migrates event retention to observability; database opening deletes only the retired state keys. Native logs remain read-only inputs.
