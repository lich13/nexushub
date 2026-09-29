# Current status

- Delivered version: `1.1.9`; the official macOS App/helper and cloud webd are installed and accepted.
- Baseline: clean `1.1.8`, local and remote commit `902109bb2b125d922f9f2231abb9d4f6bcbae698`.
- Release implementation: `03ee1ce99616f2baaf48f8966450adbd6e9f68f9`, tagged `v1.1.9`. Acceptance records use normal follow-up commits.
- Scope: Codex native asynchronous-question Bark detection while work continues. No new RPC, database table or native-session mutation. Preserve existing presentation, providers and settings.
- Keep six current Markdown documents. Private deployment inputs and actual session content remain outside Git.

## Implementation

Shared native question normalization supports synchronous question text, asynchronous titles and both option formats. Async acknowledgements, commentary, other commands and normal completion do not answer a question. Complete native replies match the original call and question index; partial answers leave other questions pending. Explicit cancellation and a new task retire the prior question.

Hook, monitor, passive scan and Stop share authoritative transcript rechecks and original per-call delivery identities. The async monitor runs without an open App or listening port. Existing settings hold a first-enable baseline and persistent atomic delivery claims; historical questions are not replayed, partial answers do not repeat pushes, and failed delivery remains recorded. Only NexusHub-owned Hook entries are upgraded.

## Validation and delivery

Local gates passed: root Rust fmt/test/Clippy (578 tests), standalone Tauri fmt/test/Clippy (64 tests), frozen WebUI install, typecheck, 264 unit tests, server/Tauri builds, contract/install guards, privacy and diff checks. Browser coverage totals 166 passing cases: 150 passed initially; 16 Chromium workspace cases passed after making the classic-scrollbar test fixture explicit on macOS. Product scrolling styles are unchanged.

Native read-only verification located the reported asynchronous call and correctly classified it as answered. Its original transcript prefix remains unchanged; later native events may continue to append. No private question content enters source or fixtures.

- [Implementation CI](https://github.com/lich13/nexushub/actions/runs/36554877847): frontend, backend and macOS Tauri passed for the release SHA.
- [Release workflow](https://github.com/lich13/nexushub/actions/runs/36555426646) and [v1.1.9 assets](https://github.com/lich13/nexushub/releases/tag/v1.1.9): seven assets, GitHub SHA-256 digests, both checksum files, updater signature and the sole `darwin-aarch64` mapping verified. Source, new Git objects and extracted payload privacy checks passed, including private values supplied outside Git.
- Official macOS App: version `1.1.9`, executable SHA-256 `3a431908dfb3fd157763b7109508cc337d99f9986ad989546e6db856bf7a3527`. Installed helper SHA-256 `481d22da8783dae13d64e0461e026dba77f5f467d894f072addceb0a05ea4844` matches the release bundle. App launch, update version and notification settings passed through the installed UI.
- Real native question: the App was closed while deployment work continued. One `reply-needed` Bark was sent in approximately 1.6 seconds, and the user confirmed receipt on the device. The subsequent native answer cleared the pending question. Restarting monitor did not duplicate delivery; monitor remains running with no listening TCP port.
- Cloud webd: version `1.1.9`, running binary SHA-256 `148608cb5d345a31de8c031b22eff5dbebbbadf03a058fd053cedeaa6c67c15f`. Authenticated HTTPS acceptance passed. Public entry and health return 200; unauthenticated sensitive RPCs return 401. Service is active with zero restarts and retains all four systemd isolation settings.
- Cloud isolated fixture: two questions produced one notification in approximately 1 second. A partial answer and normal turn completion left the remaining question pending without another push. The final answer cleared pending state; historical question/answer content remained readable. The dedicated database row, event, dedupe state and native fixture file were then removed; authenticated search confirmed no remaining task. This fixture is separate from the real device-receipt acceptance.

NexusHub configuration, cloud configuration/credentials and unrelated Hook entries were preserved. Only managed question Hook matchers were upgraded. Local Codex configuration changed during the live acceptance interval; its current file was retained, valid TOML and enabled Hooks were rechecked, and byte-for-byte preservation is not claimed for that concurrently used native configuration. Three pre-existing Grok directories lack `updates.jsonl`; their source data and unchanged Grok reader remain outside this Codex repair.

## Cleanup

Removed this task's root/Tauri build directories, generated WebUI distribution, downloaded/extracted releases, browser artifacts and macOS `1.1.8` runtime recovery copy. This removal measured 8,748,483,054 logical bytes, 7,141,765,120 allocated bytes and a 6,910,341,120-byte increase in local available space. Allocation and free-space change differ on APFS; concurrent activity can also affect the latter.

Cloud staging and its `1.1.8` runtime recovery archive were removed: 30,218,858 logical bytes and 30,322,688 allocated bytes, matching the measured increase in available space. The isolated native fixture additionally released 2,767 logical bytes / 4,096 allocated bytes. Service and public health were checked again after removal. Remaining small task logs and delivery inputs are removed after the final status commit's CI.

Keep the installed App/helper, cloud service, native user sessions, databases, credentials, production settings, monitor and existing dependency stores. No new report, screenshot archive or permanent runtime backup is retained.
