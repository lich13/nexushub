# Architecture

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

## Probe and notifications

Codex errors are selected from canonical main-task identities. Grok requires a primary native turn ending; Pi requires the current branch, a stopped assistant and no pending tool call. Unknown Pi failure state never becomes a failure notification. Persistent delivery claims prevent duplicate sends after restart; first enable establishes a baseline.

## Release matrix

CI runs frontend, backend and macOS Tauri checks. Release guard checks the tag/version, contract, privacy and matching successful CI through the Actions API. Release packaging has webd Linux, macOS ARM64 and aggregate stages. The server artifact is `nexushub-webd-linux-x86_64.tar.gz`; the updater manifest contains only `darwin-aarch64`, and the webd tarball is a server asset rather than a desktop updater entry.

Future edits update these six documents and use normal Git commits.
