# Current status

- Target version: `1.1.6`; implementation and acceptance are in progress.
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

Release, formal App/cloud acceptance and task cleanup are pending.
