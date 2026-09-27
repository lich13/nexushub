# Current status

- Current version: `1.1.7`; released, installed on macOS and deployed to the cloud service.
- Baseline: `1.1.6`, commit `0a6f24f346078d09484d50c5dd63321b8df0d97b`; local main and origin/main matched with a clean working tree.
- Scope: preserve historical tool positions and provide compact, provider-labelled activity groups for Codex, Grok and Pi. No new RPC, database field or native file changes.
- Exactly six current Markdown documents remain. Real sessions and explicit deployment inputs stay outside Git.

## Implementation

Codex no longer discards completed tools before pagination. Paired outputs retain the call's chronological position, including parallel results and pending calls. Chat and action compaction remains, with markers preserving boundaries around activity.

All providers group adjacent tools between replies. Summaries remain visible when complete groups are closed; running/failed groups start open, except instruction-only groups. Rows show tool/input previews and independently disclose output, including Grok native titles and Pi standalone Bash command text. Stable session/call identities preserve explicit choices across polling, appends, themes and mobile navigation.

The WebUI maps the existing camelCase block-page response into its domain model on both runtimes. Earlier-page loading now inserts the returned blocks instead of discarding them, and tail refreshes do not restore a pagination cursor after all history is loaded.

## Validation and delivery

Local Rust workspace and standalone Tauri formatting, tests and Clippy passed. WebUI frozen install, typecheck, 246 unit tests, server/Tauri builds and 142 Chromium/WebKit browser tests passed. The browser checks cover all three providers, 90 historical tools, native camelCase pagination with viewport anchoring, standalone Bash input/output, disclosure persistence, mobile layout, theme changes, reduced motion and bounded scrolling. Contract, installation, privacy and diff checks passed.

Release `v1.1.7` points to `0a7ea0943b98047301d529c78a451ba86b7d8fcb`. Matching frontend, backend and macOS Tauri checks passed in [CI 36330193734](https://github.com/lich13/nexushub/actions/runs/36330193734); [Release 36330463815](https://github.com/lich13/nexushub/actions/runs/36330463815) published the [seven official assets](https://github.com/lich13/nexushub/releases/tag/v1.1.7). All seven GitHub digests, both checksum files, the updater signature and the sole `darwin-aarch64` mapping were verified. DMG verification and unpacked-payload privacy checks passed. The macOS app retains the existing ad-hoc signing policy; updater signature validation is separate from Apple notarization.

Official macOS acceptance confirmed real Codex history, visible interleaved activity summaries, command disclosure and refresh persistence. Pi used an isolated native v3 fixture because no existing local Pi history was available; paired tools and standalone Bash retained their input/output and explicit disclosure choices. Authenticated HTTPS acceptance used real Codex and Grok histories. Loading an earlier Codex page increased activity groups from 23 to 48 and content height by 8,938 pixels while the original viewport anchor moved only 0.5 pixels. Grok retained 30 activity groups, native command titles and expanded input/output after refresh. Neither web view overflowed the outer page.

Installed app and helper bytes match the verified release payload. The cloud binary and all four WebUI files match their release payload; service health is successful, restart count is zero and recent service error count is zero. Local/cloud configuration hashes are unchanged. Systemd retains `ProtectHome=read-only`, `ProtectSystem=full`, `PrivateTmp=yes` and `NoNewPrivileges=yes`. After quitting the macOS app, its monitor remained running with no TCP listener.

Verified runtime SHA-256:

- macOS app executable: `239506d3ad84fe7aee913ab740d6db62c92905c313947b669dc4c850a10ba5c9`.
- macOS helper: `8c3e0587ce5aa55fb4ece36536d35334951a1a35e695b970d9dfd556fb584b31`.
- Linux webd executable: `b1189e3a641744a336006dcb9f464c79ab5f61c0cd316236098f5a91ba0159b4`.

## Cleanup and retained state

The isolated Pi fixture was removed after confirming zero references in the native state and NexusHub databases. Task-created Rust/Tauri targets, frontend output, browser test artifacts, downloaded/extracted release assets, diagnostic staging and both temporary runtime recovery archives were removed and their absence checked. This removed 6,464,373,253 bytes of logical file content and 5,962,997,760 bytes of allocated disk usage. Dependencies, official applications/services, configuration, credentials, databases and user sessions remain.

The removed recovery archives were the single task copies of the previous macOS runtime (11,284,267 bytes; SHA-256 `723e63067c10dbc13493f80c3950f1fdb5e7aa1e3b84c175d46281c4e4a433c1`) and cloud runtime (6,409,385 bytes; SHA-256 `2922178ecafe830c9e1506398dfd55055e246294825b924f51b76a3efc314849`). No task recovery copy remains. Existing attachments, Plan export, memory filtering, AGENTS.md folding, file paths, rename, batch management and Probe remain regression boundaries. No provider mutations were used on real user sessions.
