# Architecture — 1.2.1

## Boundaries

`desktop_embedded_tauri` owns the React UI; `linux_server_api` exposes only the shared core business API. Components call query/domain helpers, typed transport and `NexusHubUseCases`; provider readers and controlled effects remain behind the facade. HTTP handlers and Tauri commands are thin adapters.

| Area | Responsibility |
| --- | --- |
| Codex | Official state/index/rollout/log reads, identity, native rename, archive/restore and scoped archived deletion |
| Grok / Pi | Native session discovery, branch-aware history, identity, activity checks, rename and scoped deletion |
| Probe | Error detection, provider terminal evidence, cursors, dedupe, redaction, retention and Bark delivery |
| Database/config | NexusHub settings, events, deliveries and migration; Codex native data stays outside this schema |
| WebUI | Shared visual contract, provider queries, pure execution-group view models and Plan filename/content preparation |
| Packaging | macOS ARM64 Tauri app/updater and Linux x86_64 headless webd |

NexusHub Goal DTOs, RPC handlers, scheduler, recovery retries and `codex_thread_goals` storage are retired. The old command names are unavailable tombstones only. The database migration rebuilds `probe_error_incidents` without recovery columns and drops the Goal table while retaining incident records.

## Shared changes

1. Update `contracts/nexushub-contract.json` and schema.
2. Implement core DTO/use-case and the Linux/Tauri adapters.
3. Add WebUI query/domain behavior and tests.
4. Run contract, privacy, Rust, WebUI and install gates.

Batch actions use explicit provider keys and return per-item preview/execute results. Files are isolated before deletion and indexes are replaced atomically. Failures retain a recovery path and are not cascaded to unselected records.

## Activity timeline

Codex retains completed tool blocks and positions paired results at the native call position. Existing detail/block pagination bounds responses; chat/action compaction preserves separators around tools. WebUI normalizes all providers into adjacent activity groups with stable session/call identities. Completion closes default disclosures, while explicit choices survive polling and remounts. Legacy history summaries remain readable. No RPC, native file format or database migration is added.

## Visible Markdown

`visibleMarkdown.ts` removes structured memory metadata by source range before rendering and export. CommonMark code positions protect literal examples; a rendering guard also removes metadata elements. Copy buttons, thread previews and Plan filenames/downloads use the same cleaner. Instruction-file recognition feeds native disclosures and execution groups. A bounded in-memory map scoped by provider, session and activity preserves explicit disclosure choices; no database or native session file stores this UI state.

## User message attachments

Provider readers retain a presentation-only user message with a stable native identity, literal request text and attachment descriptors. The Codex reader pairs native image parts with attachment markers; Grok groups chunks within a user-message boundary; Pi groups text and images within the active native entry. Native files are never rewritten.

The shared frontend user-message view model splits confirmed native question replies and AGENTS.md sections from ordinary literal text. Native question IDs remain internal keys; answer-copy and question disclosure operate independently. All three renderers supply the original event/block identity for legacy messages without a user-message DTO. Instruction and question disclosures reuse the bounded session-scoped choice store. This is a presentation change without a new backend contract or migration.

`sessions.attachmentRead` is an authenticated shared read through `NexusHubUseCases`, with thin HTTP/Tauri adapters. It accepts provider, session key, message ID and attachment ID, re-resolves the source and checks file identity. PNG/JPEG/WebP/GIF previews are limited to 20 MiB each. Images are loaded near the viewport; the frontend cache is bounded and isolated by machine and cleared when the active connection changes. Remote-only references are not fetched. Polling contains descriptors, not image data.

## Files, settings and notifications

File links are parsed from the original Markdown target before browser URL resolution. The shared path component strips line/column suffixes, resolves relative paths against the selected thread workspace, copies paths for remote threads, and delegates Finder reveal to the existing Tauri opener on macOS.

Settings contain `更新与维护` and `远程连接`. `system.status` is a retired tombstone; `system.capabilities` returns API protocol version, host surface and the capability matrix. Updates and cleanup retain their own dry-run records.

Codex final replies are classified locally for explicit user feedback requests after a verified terminal turn. The monitor, Stop Hook and completion path share the `assistant_question` source and persistent delivery claims. Existing provider completion/error rules remain unchanged.

## Probe and notifications

Codex errors are selected from canonical main-task identities. Grok requires a primary native turn ending; Pi requires the current branch, a stopped assistant and no pending tool call. Unknown Pi failure state never becomes a failure notification. Persistent delivery claims prevent duplicate sends after restart; first enable establishes a baseline.

## Release matrix

CI runs frontend, backend and macOS Tauri checks. Release guard checks the tag/version, contract, privacy and matching successful CI through the Actions API. Release packaging has webd Linux, macOS ARM64 and aggregate stages. The server artifact is `nexushub-webd-linux-x86_64.tar.gz`; the updater manifest contains only `darwin-aarch64`, and the webd tarball is a server asset rather than a desktop updater entry.

Future edits update these six documents and use normal Git commits.

Native Codex question parsing lives in `codex/user_input.rs`. It normalizes synchronous `question` and asynchronous `title` fields and string/object options. The async tracker pairs original turn/call/question identities with complete native reply envelopes and handles explicit cancellation or replacement. The webd asynchronous monitor scans independently of list status and shares the sender recheck with Hook, passive and Stop events. Existing settings hold an enablement baseline and atomic per-call delivery records, so first startup skips history, partial answers do not create new identities, and restarts or TTL expiry do not resend confirmed calls. No RPC or database table is added.

## Remote transport and retirement

Desktop `remote.*` commands own connection preferences and Keychain access. They never enter the Linux dispatcher. React sends a command, arguments and connection revision through native IPC; Rust uses a fixed registry allowlist, HTTPS, disabled redirects and a 32 MiB response bound, enough for the existing 20 MiB image limit. On first remote use the protocol is verified; stale responses cannot cross a revised connection. No credential is returned to React.

A revision-scoped QueryClient and workspace lifetime isolate mutations, previews, paging and scroll; attachment keys, title overrides and disclosures include machine identity. Local updater and Plan saving stay local. Remote path links never reach Finder.

Server authentication accepts one `x-api-key`, stores only its SHA-256 digest and uses constant-time comparison. Failed authorization is bounded and audited without credentials; valid credentials are not locked out by failed attempts. CLI rotation invalidates the previous Key and revocation denies all business RPCs. Old authentication/security actions and threadEvents are unavailable tombstones.

Opening only the NexusHub database drops administrators, web sessions and Turnstile state, removes retired settings, erases freed pages and checkpoints/truncates WAL. Business audit, jobs, Bark encryption and notification state survive. Config migration removes retired web keys without replacing unrelated settings. Neither native Codex storage nor provider session formats change. Linux packages contain no React assets; the reverse proxy serves RPC and health only.

`plans.save` is a desktop-only native export to Downloads. It validates a bounded Markdown filename/content and uses exclusive file creation; it never traverses the remote HTTP transport.
