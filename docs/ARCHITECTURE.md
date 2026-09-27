# Architecture — 1.1.7

## Boundaries

`desktop_embedded_tauri` and `linux_server_webui` share the WebUI and core. Components call query/domain helpers, typed transport and `NexusHubUseCases`; provider readers and controlled effects remain behind the facade. HTTP handlers and Tauri commands are thin adapters.

| Area | Responsibility |
| --- | --- |
| Codex | Official state/index/rollout/log reads, identity, native rename, archive/restore and scoped archived deletion |
| Grok / Pi | Native session discovery, branch-aware history, identity, activity checks, rename and scoped deletion |
| Probe | Error detection, provider terminal evidence, cursors, dedupe, redaction, retention and Bark delivery |
| Database/config | NexusHub settings, events, deliveries and migration; Codex native data stays outside this schema |
| WebUI | Shared visual contract, provider queries, pure execution-group view models and client-side Plan export |
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

`sessions.attachmentRead` is an authenticated shared read through `NexusHubUseCases`, with thin HTTP/Tauri adapters. It accepts provider, session key, message ID and attachment ID, re-resolves the source and checks file identity. PNG/JPEG/WebP/GIF previews are limited to 20 MiB each. Images are loaded near the viewport; the frontend cache is bounded and cleared at logout. Remote-only references are not fetched. Polling contains descriptors, not image data.

## Files, settings and notifications

File links are parsed from the original Markdown target before browser URL resolution. The shared path component strips line/column suffixes, resolves relative paths against the selected thread workspace, copies the server path on web, and delegates Finder reveal to the existing Tauri opener on macOS.

The settings surface is a single `更新与维护` page plus platform-appropriate account/security controls. `system.status` is a retired tombstone; `system.capabilities` returns only host surface and the capability matrix. Updates and cleanup retain their own dry-run records.

Codex final replies are classified locally for explicit user feedback requests after a verified terminal turn. The monitor, Stop Hook and completion path share the `assistant_question` source and persistent delivery claims. Existing provider completion/error rules remain unchanged.

## Probe and notifications

Codex errors are selected from canonical main-task identities. Grok requires a primary native turn ending; Pi requires the current branch, a stopped assistant and no pending tool call. Unknown Pi failure state never becomes a failure notification. Persistent delivery claims prevent duplicate sends after restart; first enable establishes a baseline.

## Release matrix

CI runs frontend, backend and macOS Tauri checks. Release guard checks the tag/version, contract, privacy and matching successful CI through the Actions API. Release packaging has webd Linux, macOS ARM64 and aggregate stages. The server artifact is `nexushub-webd-linux-x86_64.tar.gz`; the updater manifest contains only `darwin-aarch64`, and the webd tarball is a server asset rather than a desktop updater entry.

Future edits update these six documents and use normal Git commits.
