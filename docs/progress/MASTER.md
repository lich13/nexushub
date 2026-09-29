# Current status

- Target: `1.2.2`; implementation and local gates passed. Official release, macOS installation and cloud API upgrade remain pending. Both running baselines are `1.2.1`.
- Grok activity now validates registered native processes, PID start times and primary-turn events through a bounded incremental cache. Ended or cancelled threads stop spinning; deletion separately protects open native sessions. Registry and session files are never repaired or rewritten.
- Settings show only the current machine’s update panel with Chinese labels, normalized versions and install controls only for confirmed updates. Background jobs keep machine switching locked until their terminal result is observed.
- Local gates: 573 root Rust tests, 71 Tauri tests, fmt/Clippy, 263 WebUI unit tests, frozen install, typecheck, desktop build, and 186 Chromium/WebKit cases passed. Contract, installer, privacy and diff checks passed.
- Read-only verification of the reported real Grok session returns `recent` in both list and detail. A dedicated native TUI session returned `running` during its primary turn. Installed-App acceptance is still pending; browser fixtures are not production evidence.
- Preserve Keychain connection/API Key, provider sessions, Bark settings, both monitors and systemd isolation. Public RPC, database schema, polling intervals and API-only deployment boundaries remain unchanged. Private production values and real session content stay outside Git.
