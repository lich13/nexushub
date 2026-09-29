# Current status

- Target version: `1.1.9`; implementation and local validation are in progress. Installed macOS and cloud remain `1.1.8` until release acceptance.
- Baseline: clean `1.1.8`, local and remote commit `902109bb2b125d922f9f2231abb9d4f6bcbae698`.
- Scope: Codex native asynchronous-question Bark detection while work continues. No new RPC, database table or native-session mutation. Preserve existing presentation, providers and settings.
- Keep six current Markdown documents. Private deployment inputs and actual session content remain outside Git.

## Implementation

Shared native question normalization supports synchronous question text, asynchronous titles and both option formats. Async acknowledgements, commentary, other commands and normal completion do not answer a question. Complete native replies match the original call and question index; partial answers leave other questions pending. Explicit cancellation and a new task retire the prior question.

Hook, monitor, passive scan and Stop share authoritative transcript rechecks and original per-call delivery identities. The async monitor runs without an open App or listening port. Existing settings hold a first-enable baseline and persistent atomic delivery claims; historical questions are not replayed, partial answers do not repeat pushes, and failed delivery remains recorded. Only NexusHub-owned Hook entries are upgraded.

## Validation and delivery

Local gates passed: root Rust fmt/test/Clippy (578 tests), standalone Tauri fmt/test/Clippy (64 tests), frozen WebUI install, typecheck, 264 unit tests, server/Tauri builds, contract/install guards, privacy and diff checks. Browser coverage totals 166 passing cases: 150 passed initially; 16 Chromium workspace cases passed after making the classic-scrollbar test fixture explicit on macOS. Product scrolling styles are unchanged.

Native read-only verification located the reported asynchronous call and correctly classified it as answered. Its original transcript prefix remains unchanged; later native events may continue to append. No private question content enters source or fixtures. Release, official install, cloud deployment and real Bark device receipt are not yet complete.

## Cleanup

Task staging remains outside the repository until acceptance. User sessions, credentials, configuration, dependencies and installed services remain protected.
