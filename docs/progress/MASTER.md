# Current status

- Delivered version: `1.1.8`, installed on macOS and deployed to the cloud service.
- Baseline: clean `1.1.7`, local and remote commit `632c91479f4d5268ff59268bd06a43f9a66bcd3e`.
- Scope: native question/answer user bubbles and default folding for user AGENTS.md instruction sections across Codex, Grok and Pi. No new RPC, database field or native-session mutation.
- Keep six current Markdown documents. Private deployment inputs and actual session content remain outside Git.

## Implementation

The shared user-message view model recognizes complete native question-reply envelopes and displays each question above its literal answer. Questions start as muted one-line references and can expand; the copy action copies only the corresponding answer. Invalid or incomplete envelopes and code/quoted examples retain their original text.

User AGENTS.md sections now use the same disclosure mechanism as instruction tools and assistant sections. Native INSTRUCTIONS boundaries include internal headings without consuming a following ordinary request. Instruction bodies are loaded when expanded and scroll within a bounded area. Native user-message identities, or legacy block/event identities, preserve user choices across refresh and navigation.

## Validation and delivery

Local gates passed: frozen WebUI install, typecheck, 264 unit tests, server/Tauri builds, and 166 Chromium/WebKit browser tests. The new browser cases cover all providers, light desktop/dark mobile, literal answers, answer copying and failure feedback, question expansion, instruction folding, bounded long-body scrolling, and disclosure choices after refresh, mobile navigation and theme changes. Root Rust and standalone Tauri fmt/test/Clippy, contract/install guards, privacy scans and diff checks passed.

Implementation commit: `b6b4cc3104d5d4977b0be59026ef5148b92f4412`. Its [three-job CI](https://github.com/lich13/nexushub/actions/runs/36348348875) and [Release workflow](https://github.com/lich13/nexushub/actions/runs/36348714252) passed. [v1.1.8](https://github.com/lich13/nexushub/releases/tag/v1.1.8) points to that commit. All seven downloaded assets match GitHub SHA-256 metadata; both checksum files, the updater signature, the DMG and the sole `darwin-aarch64` updater mapping were verified. The unpacked release payload passed privacy scanning.

Official macOS acceptance used an existing native Codex conversation containing the reported reply and instruction formats. The question reference, literal answer, keyboard expansion and answer-only clipboard copy passed; the clipboard matched the original answer exactly. AGENTS.md started closed, retained internal same-level headings when expanded and scrolled within its own area. Refresh preserved explicit disclosure choices. The original session fingerprint and application configuration remained unchanged.

The authenticated HTTPS entrypoint used a dedicated Grok fixture to exercise the same shared presentation path. Both replies remained ordered, including literal Markdown and newlines; the system clipboard exactly matched the selected answer. AGENTS.md started closed, retained a following ordinary request, and preserved expansion after refresh. Its expanded body was bounded to 460px with 1138px of internally scrollable content; the document remained 720px high in a 720px viewport. This cloud evidence is an isolated fixture, not pre-existing user history. Cross-provider, theme, mobile and clipboard-failure coverage comes from the browser suite.

The installed macOS app and helper both report `1.1.8` and match the official payload. App SHA-256: `64bddbf8ef7dc2315826206077bbf024700aaa69fde126795c11a3815d751f66`; helper SHA-256: `b6ae5629187e1b508a73d77e7c6f92b27d3b18287ce90ba0cc74641750ea41a2`. After a normal App quit, the new monitor remained running with no TCP listeners.

The cloud binary reports `1.1.8` with SHA-256 `555c17d6f40faa98508a579d72889873928dc797bbd799943de407281ef61065`; the binary and all deployed WebUI files match the release payload. The service remained active/running with zero restarts and a healthy response after acceptance cleanup. Configuration hashes and systemd hardening remained unchanged.

## Cleanup

Removed this task's root/Tauri build targets, generated WebUI output, downloads, unpacked assets, test staging and both temporary runtime recovery archives. The measured final local cleanup removed 5,246,418,387 logical bytes and 5,298,237,440 allocated bytes; filesystem free space increased by 5,042,262,016 bytes across the two removal operations. Browser test output had already been removed. No temporary mount remains.

Removed the cloud fixture's three files and its own monitor cursor after confirming zero native-session references and notification events, then removed its temporary script directory (6,845 logical bytes; 12,288 allocated bytes). The fixture and cursor were verified absent. User sessions, credentials, configuration, dependencies, the official application/service and the monitor remain. No independent acceptance report or screenshot package was retained.
