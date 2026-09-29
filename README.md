# NexusHub 1.2.3

NexusHub is a read-first workspace for Codex, Claude Code, Grok Build and Pi sessions. The React UI runs in the macOS ARM64 Tauri app. A headless Linux `webd` API manages the remote machine. Deployment hosts, domains and credentials are supplied explicitly and stay outside Git.

## Capabilities

- **Codex**: search, read, rename through the native app-server protocol, archive/restore, batch archive/restore/delete of explicit records, and copy IDs or paths.
- **Claude Code**: read existing native JSONL sessions, search, copy IDs, inspect tools and attachments, export saved plans, rename inactive verified sessions, and batch-delete selected JSONL files. Claude is never installed or configured by NexusHub.
- **Grok Build**: read native sessions, search, rename, copy the native thread ID, batch-delete selected session directories, and inspect tool activity.
- **Pi**: read native JSONL branches, search, rename through the fixed native RPC when available, copy the native thread ID, and batch-delete selected session files. Missing or uncertain activity is read-only.
- **Probe**: read-only error detection, persistent dedupe and Bark delivery. Codex, Claude Code, Grok and Pi completion notifications follow provider-specific evidence rules; Codex final replies that explicitly wait for confirmation, a choice, more information or post-action feedback use the existing `reply_needed` Bark channel. Codex native synchronous and asynchronous questions also use that channel while waiting for answers, even as other work continues. An asynchronous acknowledgement is not an answer; matched replies, cancellation and replacement determine pending state. New asynchronous monitoring starts from an enablement baseline and does not replay old questions. Pi final-failure notification remains disabled without an extension.
- **Files and UI**: Markdown file links preserve the source path. macOS reveals resolved local paths in Finder and copies the exact path beside the link; remote links copy the server path. Running sessions share one accessible spinner. Codex, Claude Code, Grok and Pi retain historical tools between their original replies and group adjacent activity, including Read/List/search and commands, with one collapsible row per call. Provider-labelled activity summaries always remain visible; expanded rows show the tool, input preview, state and output. Completed groups start closed, while active or failed items stay open. Structured memory metadata is removed from display, reply copy and Plan export while code examples and ordinary memory references remain intact. AGENTS.md file sections and tool output start collapsed, including running/failed entries; manual choices survive refresh and mobile navigation. Visible assistant replies can be copied as cleaned Markdown; Codex, Claude and Grok plans can also be copied or downloaded as `.md` files named from their titles. List rows support double-click rename when the provider allows it.

NexusHub has no Goal management or automatic recovery path. Codex native logs remain read-only inputs for error detection. The retired `codex_thread_goals` table is removed during database migration. Composer, send/steer/stop/fork, manual approvals/questions/plans, uploads and the desktop LAN WebUI remain unavailable.

Batch operations accept at most 100 explicit session keys. Preview results show scope, size, fingerprints and blockers. Execution rechecks identity, activity, symlinks and fingerprints; failures remain visible and require a new preview.

User messages share a plain-text bubble across Codex, Claude Code, Grok and Pi. Native attachment envelopes are separated from the request; images appear above the bubble and open in a keyboard-accessible preview. Native embedded images survive expired temporary file paths. Unavailable attachments remain compact file cards. Message history remains read-only.

Native question replies display a muted, expandable question above the literal answer; the neighboring copy action copies only that answer. Internal envelope fields remain hidden. User AGENTS.md sections start folded with line/byte counts and retain manual disclosure choices. Ordinary prose, quoted/code examples and unrecognized envelopes retain their source text.

## Machines

Use the machine selector above navigation to choose 本机 or 腾讯云. Configure the remote HTTPS address and independent administrator API Key in 设置 → 远程连接. The App verifies the API protocol before saving; only the address and selection are stored in local preferences, and the Key lives in macOS Keychain. Removing the connection removes its Keychain item.

Reads, session changes, cleanup, Probe and execution records follow the selected machine. A failed connection stays on its selected target. Switching discards unfinished previews and selection, isolates caches and drops old responses; writes block switching until they return. Settings show only the selected machine’s update panel: 本机 App 更新 or 腾讯云服务更新. Updates appear only when a newer version is confirmed; both targets use the same Chinese controls, and the server retains backup cleanup. Each machine runs its own monitor and Bark sender independently of the App.

There is no hosted website, browser login, Cookie/CSRF or Turnstile. Cloud RPC requires `x-api-key`; health remains public, and old pages return 404. Native provider data is preserved.

Grok running indicators require a live native process with verified registration identity and an unfinished primary turn. Completed or cancelled turns display their timestamp; uncertain activity is explicit. An open native session still blocks deletion even after its turn ends.

## Development

```bash
corepack pnpm@11.0.8 --dir webui install --frozen-lockfile
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui typecheck
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build:tauri
corepack pnpm@11.0.8 --dir webui test:browser
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
bash scripts/test-install-script.sh
```

`src-tauri` is an independent Cargo workspace with its own lockfile. Tests use disposable provider data and never delete user sessions.

## Runtime paths

| Surface | Data and logs |
| --- | --- |
| macOS app | `~/Library/Application Support/NexusHub/`, `~/Library/Logs/NexusHub/` |
| Linux server | `/etc/nexushub-webd/`, `/var/lib/nexushub-webd/`, `/var/log/nexushub-webd/` |

Codex uses its official local state DB, session index, rollouts and logs. Claude reads `CLAUDE_CONFIG_DIR/projects` or `~/.claude/projects`. Only inactive, identified 2.x transcripts allow an appended native `custom-title`; native title refresh and persistence depend on Claude. Unknown records remain readable and block management. Explicit completion, terminal failure and unresolved `AskUserQuestion` or identity-bound permission records can notify through Bark; absent or unbound permission records and ordinary prose do not imply a reply request. Grok and Pi roots are discovered from their standard environment variables or user directories. Missing provider directories produce an empty state and do not install software or write configuration.

## Release and deployment

Release `v1.2.3` publishes seven files: macOS DMG and checksum, macOS updater archive and signature, `latest.json` with only `darwin-aarch64`, and the Linux webd tarball and checksum. Linux Tauri desktop packages and desktop updater entries are retired. The server tarball is never placed in `latest.json`. Settings expose one `更新与维护` surface; `system.status` is retired and `system.capabilities` is the only runtime capability query.

```bash
bash scripts/package-darwin-arm64.sh
bash scripts/package-webd-linux-x86_64.sh
```

Use the exact approved tag and verify sidecar hashes before installation. Cloud deployment keeps `HOST`, domain and archive paths explicit:

```bash
NEXUSHUB_DOMAIN=panel.example bash scripts/deploy-cloud.sh SSH_HOST /absolute/staging/nexushub-webd-linux-x86_64.tar.gz
```

See [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md) for systemd isolation, writable session roots, upgrades and recovery.

## Current documents

- [AGENTS.md](AGENTS.md): development constraints and gates.
- [DESIGN.md](DESIGN.md): shared visual and interaction rules.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): module boundaries and contracts.
- [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md): headless deployment.
- [docs/progress/MASTER.md](docs/progress/MASTER.md): current status and evidence.

Keep exactly these six Markdown documents. Future work uses normal Git commits.

Plan Markdown is saved to the local Downloads folder. Existing files are kept; duplicate names receive a numbered suffix.
