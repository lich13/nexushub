# Current status

- Target version: `1.1.8`; implementation and local gates complete, release delivery pending.
- Baseline: clean `1.1.7`, local and remote commit `632c91479f4d5268ff59268bd06a43f9a66bcd3e`.
- Scope: native question/answer user bubbles and default folding for user AGENTS.md instruction sections across Codex, Grok and Pi. No new RPC, database field or native-session mutation.
- Keep six current Markdown documents. Private deployment inputs and actual session content remain outside Git.

## Implementation

The shared user-message view model recognizes complete native question-reply envelopes and displays each question above its literal answer. Questions start as muted one-line references and can expand; the copy action copies only the corresponding answer. Invalid or incomplete envelopes and code/quoted examples retain their original text.

User AGENTS.md sections now use the same disclosure mechanism as instruction tools and assistant sections. Native INSTRUCTIONS boundaries include internal headings without consuming a following ordinary request. Instruction bodies are loaded when expanded and scroll within a bounded area. Native user-message identities, or legacy block/event identities, preserve user choices across refresh and navigation.

## Validation and delivery

Local gates passed: frozen WebUI install, typecheck, 264 unit tests, server/Tauri builds, and 166 Chromium/WebKit browser tests. The new browser cases cover all providers, light desktop/dark mobile, literal answers, answer copying and failure feedback, question expansion, instruction folding, bounded long-body scrolling, and disclosure choices after refresh, mobile navigation and theme changes. Root Rust and standalone Tauri fmt/test/Clippy, contract/install guards, privacy scans and diff checks passed.

Formal delivery remains pending: matching three-job CI, seven verified release assets, official macOS installation, explicit cloud deployment and real-entrypoint acceptance. Prior delivery evidence is available in Git. Task staging and temporary runtime recovery files are removed after acceptance; user data, dependencies and production configuration remain.
