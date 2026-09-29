# Current status

- Delivered: `1.2.1`, installed macOS App/helper and deployed Linux API verified from official assets. The original `1.2.0` migration is complete; installed-App acceptance found a WebKit download stall, fixed by native Plan saving in `1.2.1`.
- Scope: cloud WebUI/login/Turnstile retired; independent administrator API Key, Keychain connection and 本机/腾讯云 switching are active. Normal incremental Git history continues.
- Implementation: `c877051381315ab0fd66b2f92219ae854eafadef`; frontend, backend and macOS Tauri [CI passed](https://github.com/lich13/nexushub/actions/runs/36578867107).
- [Release v1.2.1](https://github.com/lich13/nexushub/releases/tag/v1.2.1) and its [release workflow](https://github.com/lich13/nexushub/actions/runs/36579888341) succeeded. All seven assets match GitHub SHA-256 digests; both checksums, updater signature and the sole `darwin-aarch64` mapping passed.
- Installed App binary SHA-256: `92553523a89a55ff104a41ba63cf1510da7c94bea9984991f7a37ed4a8caf127`.
- Installed helper SHA-256: `3c47c18bbb66d80776268158fabe2b480893188b51b05748045a957e17bbe534`.
- Deployed/running Linux binary SHA-256: `9e89a6beba2da47e7ed91e71f989bcfaf62981e10533c85123d9e72a867285d5`.

## Verification

- Rust: 566 root-workspace tests and 71 Tauri tests; formatting and Clippy passed. WebUI: frozen install, typecheck, 256 unit tests, desktop build and full Chromium/WebKit CI passed; the native Plan correction also passed 22 targeted Plan/connection browser cases. Contract, install/migration and packaging checks passed.
- Source, Git objects/identities and both unpacked release payloads passed privacy scanning (1,262 checked items). Six current Markdown documents remain.
- Official App: connection save/remove/reconnect, Keychain reuse after restart and upgrade, failed connection without target fallback, same-ID data isolation, write-time switch blocking, local Finder reveal, remote path copying and image preview passed.
- Dedicated Grok sessions on both machines passed native/UI rename and scoped deletion, including remote batch deletion. Existing Codex details rendered on both machines. Both Pi roots were empty: empty-state behavior passed; native Pi session management was not exercised.
- Local and remote Plan exports saved UTF-8 Markdown into local Downloads. Three actual files matched their displayed content; repeated names gained numbered suffixes without overwriting. The App remained responsive.
- Public health returned 200; retired pages/static/login returned 404; missing Key and old Cookie returned 401. Correct/incorrect, rotated and revoked Keys were checked. Final capabilities report `linux_server_api` / API 1.
- Old web authentication tables/settings and static payloads are absent. Bark settings and shared Nginx files matched their pre-migration values; business records were preserved. Formal Key plaintext was absent from logs, configuration and database/WAL scans.
- After App closure, the local monitor remained healthy without a TCP listener; the cloud monitor continued fresh scans. systemd isolation remains enabled. Grok's native leader uses the service data directory for its socket/lock while its home remains read-only.

## Cleanup

- Removed dedicated sessions, exported test files, build/download staging, diagnostic runtime files, temporary Key files and all task-level recovery copies. No retired website/login backup was kept.
- Final cleanup removed 10,584,436,736 bytes of allocated task-file space across both machines, including three exported files. Measured available-space increases during cleanup were 8,145,682,432 bytes locally and 7,217,152 bytes on Linux; filesystem allocation/sharing makes these distinct from logical file sizes. The earlier retired web payload removal accounted for 571,520 logical bytes.
- Tracked source is 3,320,976 bytes versus 3,375,095 bytes at `1.1.9`, a net reduction of 54,119 bytes.
- Retained: official App/helper/API service, Keychain connection, production configuration and database, Bark material, provider sessions, independent monitors and existing dependencies. No production credentials, private deployment values or real session contents are recorded here.
