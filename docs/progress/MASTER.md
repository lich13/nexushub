# Current status

- Target version: `1.1.6`; implementation, release and live acceptance are complete.
- Baseline: `1.1.5`, commit `7dde809a77bd04904d51a6ad2014fed065ba80e2`; the working tree and remote main matched before implementation.
- Scope: shared literal user-message bubbles and native attachment previews for Codex, Grok and Pi. Assistant Markdown, tool groups, Plans, navigation and settings retain their existing behavior.
- Current documentation remains the six files linked from README. Private deployment inputs and real-session evidence stay outside the repository.

## Implementation

User-message presentation separates confirmed native attachment wrappers from the actual request. Images sit outside the text bubble, support keyboard preview and retain an unavailable placeholder when the source cannot be read. Native embedded image data takes precedence over temporary paths. Stable message and attachment identities support pagination and polling without exposing image data in detail responses.

`sessions.attachmentRead` is an authenticated read-only contract shared by core, Linux webd and Tauri. It resolves existing sessions, accepts no client file path, rechecks attachment identity, and limits previews to supported raster images up to 20 MiB. Native sessions and the NexusHub database schema remain unchanged.

## Required delivery evidence

Record the implementation SHA, matching frontend/backend/macOS Tauri CI, seven-asset Release checks, official macOS installation, authenticated cloud acceptance and precise task cleanup here after they complete. Real entrypoint results must be distinguished from fixtures. Existing memory filtering, AGENTS.md folding, Plan export, paths, rename and batch operations remain regression gates.

## Local validation

WebUI frozen install, typecheck and both builds passed; 243 unit tests passed. The complete Chromium/WebKit regression passed 122 cases; the final attachment suite passed 16 cases after adding retry coverage. Workspace Rust tests, Tauri tests, both Clippy checks, formatting, contract/install checks, privacy scan and diff checks passed. The screenshot-related native record was inspected without storing its content in the repository: one embedded image remained readable, and neither the attachment wrapper nor image markers remained in the request body.

## Delivery evidence

- Implementation commit: `fbfff7fe93b3ec8f6ab9f15aa5a04544869e5a0a`; `main` and `v1.1.6` point to this commit.
- CI run `36298134344`: Frontend, Backend and macOS Tauri checks all passed. Release run `36298419907` passed its boundary guard, macOS asset, Linux webd asset and publish jobs.
- `v1.1.6` is a non-draft, non-prerelease release with seven assets. GitHub digests, DMG/webd checksum sidecars, the `darwin-aarch64` updater mapping and the updater signature were independently verified.
- The signed macOS asset was installed as `/Applications/NexusHub.app`; the bundle and helper report `1.1.6`. After closing the app, the monitor remained running and no NexusHub listening socket was present.
- The cloud service was upgraded through the explicit deployment script. The service reports `nexushub-webd 1.1.6`, is active and enabled, and `/healthz` returned `{"ok":true}`. The authenticated web entrypoint rendered a dedicated fixture with literal user line breaks, a readable embedded PNG thumbnail, an unavailable missing-file card, preview open, and Escape close. The fixture row, session file and index entry were removed afterward.
- Local gates and browser regression remain green: frozen install, typecheck, build, Tauri build, Rust/Tauri tests and Clippy, 124 WebUI/browser cases, contract/install/privacy checks, and `git diff --check`. The screenshot-related native record was inspected without storing its content in the repository.
- Task-only release downloads, extracted payloads, recovery materials and test fixtures are outside the repository and are removed after the final evidence commit. Formal app, cloud service, user sessions, credentials and production configuration remain.
