# Current status

- In progress: `1.2.3` adds Claude Code to the existing local/remote API architecture. The installed baseline remains `1.2.2` until release acceptance.
- Core reading, guarded native-title append, single-file deletion, shared batch/attachment actions, paged UI and Bark provider settings are implemented. Unknown records remain readable and block management; uncertain activity never proves running or permits a mutation.
- Local gates passed: Rust workspace 588 tests, Tauri 71 tests, frontend 264 tests, Chromium/WebKit 196 checks, formatting, Clippy, contracts, installer boundaries and privacy checks. The isolated installed Claude 2.1.284 CLI produced a native transcript; reading and native-title append passed against that file. Repository fixtures contain only invented data. No Claude installation or native configuration change is part of delivery.
- Remaining: exact-SHA CI/release, seven-asset verification, official macOS installation, Tencent API update, real entrypoint acceptance and task cleanup.
- Preserve native sessions, Keychain/API credentials, Bark configuration, independent monitors and API-only remote access. No hosted website or execution controls are added.
